use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::config::{Config, InitStyle};

pub const ROOT: usize = 0;

#[derive(Clone, Copy)]
pub enum NodeKind {
    Container,
    Script,
    Model,
}

pub struct Node {
    pub name: String,
    pub class: String,
    pub file: Option<PathBuf>,
    pub kind: NodeKind,
    pub children: Vec<usize>,
}

impl Node {
    fn new(name: String, class: String, file: Option<PathBuf>, kind: NodeKind) -> Self {
        Node {
            name,
            class,
            file,
            kind,
            children: Vec::new(),
        }
    }
}

pub struct Project {
    pub nodes: Vec<Node>,
}

pub fn build_tree(config: &Config, input_dir: &Path) -> Result<Project> {
    let root = Node::new(String::new(), "DataModel".to_string(), None, NodeKind::Container);

    let mut builder = Builder {
        config,
        nodes: vec![root],
        by_path: HashMap::new(),
    };

    for mapping in &config.input_mappings {
        let dir = input_dir.join(&mapping.fs_path);
        if dir.is_dir() {
            builder.walk_dir(&dir, &mapping.roblox_path, &[], true)?;
        }
    }

    Ok(Project {
        nodes: builder.nodes,
    })
}

struct Builder<'a> {
    config: &'a Config,
    nodes: Vec<Node>,
    by_path: HashMap<Vec<String>, usize>,
}

impl Builder<'_> {
    fn add_node(&mut self, parent: usize, node: Node) -> usize {
        self.nodes.push(node);
        let index = self.nodes.len() - 1;
        self.nodes[parent].children.push(index);
        index
    }

    fn ensure(&mut self, path: &[String]) -> usize {
        if path.is_empty() {
            return ROOT;
        }

        if let Some(index) = self.by_path.get(path) {
            return *index;
        }

        let parent = self.ensure(&path[..path.len() - 1]);
        let name = path[path.len() - 1].clone();
        let class = if path.len() == 1 || is_nested_service(path) {
            name.clone()
        } else {
            "Folder".to_string()
        };

        let index = self.add_node(parent, Node::new(name, class, None, NodeKind::Container));
        self.by_path.insert(path.to_vec(), index);
        index
    }

    fn walk_dir(
        &mut self,
        dir: &Path,
        base: &[String],
        rel: &[String],
        is_mapping_root: bool,
    ) -> Result<()> {
        let dir_name = match dir.file_name().and_then(|name| name.to_str()) {
            Some(name) => name.to_string(),
            None => return Ok(()),
        };

        let mut base = base.to_vec();
        let mut routed = false;
        if !is_mapping_root
            && let Some(target) = self.config.route_for(&dir_name)
        {
            base = target.to_vec();
            routed = true;
        }
        let mut rel = rel.to_vec();

        let backing = if routed {
            None
        } else {
            match find_backing_script(dir, &dir_name, self.config.init_style) {
                Some(path) => Some((path, NodeKind::Script)),
                None => find_backing_model(dir, &dir_name, self.config.init_style)
                    .map(|path| (path, NodeKind::Model)),
            }
        };

        if !routed && (!is_mapping_root || backing.is_some()) {
            rel.push(dir_name.clone());
        }
        let mut content_path = base.clone();
        content_path.extend(rel.iter().cloned());

        if let Some((path, kind)) = &backing {
            let parent = self.ensure(&content_path[..content_path.len() - 1]);

            let class = match kind {
                NodeKind::Script => {
                    let file_name = path.file_name().unwrap().to_str().unwrap();
                    script_info(file_name).1.to_string()
                }
                _ => String::new(),
            };

            let node = Node::new(dir_name.clone(), class, Some(path.clone()), *kind);
            let index = self.add_node(parent, node);
            self.by_path.insert(content_path.clone(), index);
        }

        let mut entries = Vec::new();
        let mut sub_dirs = Vec::new();
        for entry in
            fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))?
        {
            let path = entry?.path();
            if path.is_dir() {
                sub_dirs.push(path.clone());
            }
            entries.push(path);
        }

        entries.sort_by(|a, b| natural_cmp(&entry_name(a), &entry_name(b)));

        let backing_path = backing.as_ref().map(|(path, _)| path.as_path());

        for path in entries {
            if path.is_dir() {
                self.walk_dir(&path, &base, &rel, false)?;
                continue;
            }

            if Some(path.as_path()) == backing_path {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let file_name = file_name.to_string();

            let is_script = is_luau_file(&path);
            let is_model = is_rbxm_file(&path);
            if !is_script && !is_model {
                continue;
            }

            if self.config.init_style == InitStyle::Sibling
                && claimed_by_sibling_dir(self.config, &file_name, &sub_dirs)
            {
                continue;
            }

            let parent_path = match self.config.route_for(&file_name) {
                Some(target) => {
                    let mut routed_path = target.to_vec();
                    routed_path.extend(rel.iter().cloned());
                    routed_path
                }
                None => content_path.clone(),
            };
            let parent = self.ensure(&parent_path);

            if is_script {
                let (name, class, _) = script_info(&file_name);
                self.add_node(
                    parent,
                    Node::new(name.to_string(), class.to_string(), Some(path), NodeKind::Script),
                );
            } else {
                let stem = file_name.strip_suffix(".rbxm").unwrap_or(&file_name);
                self.add_node(
                    parent,
                    Node::new(stem.to_string(), String::new(), Some(path), NodeKind::Model),
                );
            }
        }

        Ok(())
    }
}

fn entry_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_string()
}

fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    natural_cmp_chars(&a.to_lowercase(), &b.to_lowercase()).then_with(|| a.cmp(b))
}

fn natural_cmp_chars(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut i = 0;
    let mut j = 0;

    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let start_i = i;
            let start_j = j;
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }

            let num_a: String = a[start_i..i].iter().collect();
            let num_b: String = b[start_j..j].iter().collect();
            let trimmed_a = num_a.trim_start_matches('0');
            let trimmed_b = num_b.trim_start_matches('0');

            let order = trimmed_a
                .len()
                .cmp(&trimmed_b.len())
                .then_with(|| trimmed_a.cmp(trimmed_b));
            if order != Ordering::Equal {
                return order;
            }
        } else {
            if a[i] != b[j] {
                return a[i].cmp(&b[j]);
            }
            i += 1;
            j += 1;
        }
    }

    a.len().cmp(&b.len())
}

fn claimed_by_sibling_dir(config: &Config, file_name: &str, sub_dirs: &[PathBuf]) -> bool {
    let stem = backing_stem(file_name);

    for sub_dir in sub_dirs {
        let Some(dir_name) = sub_dir.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if dir_name.eq_ignore_ascii_case(stem) && config.route_for(dir_name).is_none() {
            return true;
        }
    }

    false
}

fn backing_stem(file_name: &str) -> &str {
    if let Some(stem) = file_name.strip_suffix(".server.luau") {
        return stem;
    }
    if let Some(stem) = file_name.strip_suffix(".client.luau") {
        return stem;
    }
    if let Some(stem) = file_name.strip_suffix(".luau") {
        return stem;
    }
    if let Some(stem) = file_name.strip_suffix(".rbxm") {
        return stem;
    }
    file_name
}

fn find_backing_model(dir: &Path, dir_name: &str, style: InitStyle) -> Option<PathBuf> {
    let path = match style {
        InitStyle::DirectoryName => dir.join(format!("{dir_name}.rbxm")),
        InitStyle::InitFile => dir.join("init.rbxm"),
        InitStyle::Sibling => dir.parent()?.join(format!("{dir_name}.rbxm")),
    };

    if path.is_file() { Some(path) } else { None }
}

pub fn find_backing_script(dir: &Path, dir_name: &str, style: InitStyle) -> Option<PathBuf> {
    let (location, base) = match style {
        InitStyle::DirectoryName => (dir.to_path_buf(), dir_name.to_string()),
        InitStyle::InitFile => (dir.to_path_buf(), "init".to_string()),
        InitStyle::Sibling => (dir.parent()?.to_path_buf(), dir_name.to_string()),
    };

    for suffix in [".server.luau", ".client.luau", ".luau"] {
        let path = location.join(format!("{base}{suffix}"));
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

fn is_nested_service(path: &[String]) -> bool {
    path.len() == 2
        && path[0] == "StarterPlayer"
        && (path[1] == "StarterPlayerScripts" || path[1] == "StarterCharacterScripts")
}

pub fn is_luau_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("luau")
}

pub fn is_rbxm_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("rbxm")
}

pub fn script_info(file_name: &str) -> (&str, &str, Option<u32>) {
    if let Some(name) = file_name.strip_suffix(".server.luau") {
        (name, "Script", Some(1))
    } else if let Some(name) = file_name.strip_suffix(".client.luau") {
        (name, "Script", Some(2))
    } else if let Some(name) = file_name.strip_suffix(".luau") {
        (name, "ModuleScript", None)
    } else {
        (file_name, "ModuleScript", None)
    }
}

pub fn is_script_class(class_name: &str) -> bool {
    matches!(class_name, "Script" | "LocalScript" | "ModuleScript")
}

pub fn should_skip_sync(class_name: &str, name: &str) -> bool {
    matches!(class_name, "Terrain" | "Camera") || matches!(name, "Terrain" | "Camera")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("fuse-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn collect_paths(tree: &Project, index: usize, prefix: &str, out: &mut Vec<String>) {
        for child in &tree.nodes[index].children {
            let node = &tree.nodes[*child];
            let path = if prefix.is_empty() {
                node.name.clone()
            } else {
                format!("{prefix}/{}", node.name)
            };

            let kind = match node.kind {
                NodeKind::Container => "container",
                NodeKind::Script => "script",
                NodeKind::Model => "model",
            };

            out.push(format!("{kind}:{path}"));
            collect_paths(tree, *child, &path, out);
        }
    }

    fn tree_paths(config: &Config, root: &Path) -> Vec<String> {
        let tree = build_tree(config, root).unwrap();
        let mut out = Vec::new();
        collect_paths(&tree, ROOT, "", &mut out);
        out.sort();
        out
    }

    #[test]
    fn routes_reshape_a_feature_layout() {
        let root = temp_root("tree-routes");

        let combat = root.join("src").join("features").join("combat");
        write(&combat.join("server").join("DamageService.luau"), "return 1");
        write(&combat.join("client").join("CombatController.luau"), "return 2");
        write(&combat.join("CombatTypes.luau"), "return 3");
        write(&combat.join("Boss.server.luau"), "print('boss')");
        write(&root.join("src").join("shared").join("Items.luau"), "return 4");
        fs::create_dir_all(root.join("src").join("empty")).unwrap();

        let config = Config::from_json(
            r#"{
                "project_name": "test",
                "mapping": { "src": "ReplicatedStorage/Source" },
                "customRoutes": {
                    "server": "ServerScriptService",
                    "client": "StarterPlayerScripts",
                    "shared": "ReplicatedStorage/Shared",
                    "*.server.luau": "ServerScriptService"
                }
            }"#,
        )
        .unwrap();

        let paths = tree_paths(&config, &root);

        assert!(paths.contains(&"script:ReplicatedStorage/Shared/Items".to_string()));
        assert!(paths.contains(
            &"script:ReplicatedStorage/Source/features/combat/CombatTypes".to_string()
        ));
        assert!(paths.contains(&"script:ServerScriptService/features/combat/Boss".to_string()));
        assert!(
            paths.contains(&"script:ServerScriptService/features/combat/DamageService".to_string())
        );
        assert!(paths.contains(
            &"script:StarterPlayer/StarterPlayerScripts/features/combat/CombatController"
                .to_string()
        ));

        for path in &paths {
            assert!(!path.ends_with("/empty"));
            assert!(!path.ends_with("/server"));
            assert!(!path.ends_with("/client"));
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn natural_cmp_orders_numeric_suffixes() {
        let mut names = vec![
            "Model_10.rbxm".to_string(),
            "Model.rbxm".to_string(),
            "Model_2.rbxm".to_string(),
            "Model_12".to_string(),
        ];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            ["Model.rbxm", "Model_2.rbxm", "Model_10.rbxm", "Model_12"]
        );
    }

    #[test]
    fn natural_cmp_is_case_insensitive_with_stable_tiebreak() {
        let mut names = vec!["SLASH_2.rbxm".to_string(), "Slash.rbxm".to_string()];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(names, ["Slash.rbxm", "SLASH_2.rbxm"]);

        assert_eq!(natural_cmp("Slash", "slash"), std::cmp::Ordering::Less);
    }

    #[test]
    fn directory_name_init_style() {
        let root = temp_root("tree-dirname");

        write(&root.join("src").join("Inventory").join("Inventory.luau"), "return {}");
        write(&root.join("src").join("Inventory").join("Helper.luau"), "return {}");

        let config = Config::from_json(
            r#"{ "project_name": "test", "mapping": { "src": "ReplicatedStorage" } }"#,
        )
        .unwrap();

        let paths = tree_paths(&config, &root);
        assert!(paths.contains(&"script:ReplicatedStorage/Inventory".to_string()));
        assert!(paths.contains(&"script:ReplicatedStorage/Inventory/Helper".to_string()));
        assert_eq!(paths.len(), 3);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn init_file_init_style() {
        let root = temp_root("tree-init");

        write(&root.join("src").join("Inventory").join("init.luau"), "return {}");
        write(&root.join("src").join("Inventory").join("Helper.luau"), "return {}");

        let config = Config::from_json(
            r#"{
                "project_name": "test",
                "mapping": { "src": "ReplicatedStorage" },
                "initStyle": "init"
            }"#,
        )
        .unwrap();

        let paths = tree_paths(&config, &root);
        assert!(paths.contains(&"script:ReplicatedStorage/Inventory".to_string()));
        assert!(paths.contains(&"script:ReplicatedStorage/Inventory/Helper".to_string()));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn sibling_init_style() {
        let root = temp_root("tree-sibling");

        write(&root.join("src").join("Inventory.luau"), "return {}");
        write(&root.join("src").join("Inventory").join("Helper.luau"), "return {}");
        write(&root.join("src").join("Loose.luau"), "return {}");

        let config = Config::from_json(
            r#"{
                "project_name": "test",
                "mapping": { "src": "ReplicatedStorage" },
                "initStyle": "sibling"
            }"#,
        )
        .unwrap();

        let paths = tree_paths(&config, &root);
        assert!(paths.contains(&"script:ReplicatedStorage/Inventory".to_string()));
        assert!(paths.contains(&"script:ReplicatedStorage/Inventory/Helper".to_string()));
        assert!(paths.contains(&"script:ReplicatedStorage/Loose".to_string()));

        let mut inventory_count = 0;
        for path in &paths {
            if path == "script:ReplicatedStorage/Inventory" {
                inventory_count += 1;
            }
        }
        assert_eq!(inventory_count, 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn directory_named_like_rbxm_is_not_a_backing_model() {
        let root = temp_root("tree-rbxmdir");

        write(
            &root
                .join("src")
                .join("Garganta")
                .join("Garganta.rbxm")
                .join("Inner.luau"),
            "return {}",
        );

        let config = Config::from_json(
            r#"{ "project_name": "test", "mapping": { "src": "ReplicatedStorage" } }"#,
        )
        .unwrap();

        let paths = tree_paths(&config, &root);
        assert!(paths.contains(&"script:ReplicatedStorage/Garganta/Garganta.rbxm/Inner".to_string()));
        for path in &paths {
            assert!(!path.starts_with("model:"));
        }

        let _ = fs::remove_dir_all(&root);
    }
}
