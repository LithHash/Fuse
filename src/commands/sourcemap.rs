use crate::{
    config::Config,
    project::{NodeKind, Project, ROOT, build_tree, should_skip_sync},
    watcher::Watcher,
};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value};
use std::{fs, path::Path, time::Duration};

struct SourceNode {
    name: String,
    class_name: String,
    file_paths: Vec<String>,
    children: Vec<SourceNode>,
}

impl SourceNode {
    fn into_json(self) -> Value {
        let mut map = Map::new();
        map.insert("name".into(), Value::String(self.name));
        map.insert("className".into(), Value::String(self.class_name));

        if !self.file_paths.is_empty() {
            let mut paths = Vec::new();
            for path in self.file_paths {
                paths.push(Value::String(path));
            }
            map.insert("filePaths".into(), Value::Array(paths));
        }

        if !self.children.is_empty() {
            let mut children = Vec::new();
            for child in self.children {
                children.push(child.into_json());
            }
            map.insert("children".into(), Value::Array(children));
        }

        Value::Object(map)
    }
}

pub fn run(config: &Config, input_dir: &Path, output_file: &Path) -> Result<()> {
    println!("Generating sourcemap for {}", config.project_name);

    if !input_dir.is_dir() {
        bail!("Sourcemap input must be a directory: {}", input_dir.display());
    }

    let project = build_tree(config, input_dir)?;

    let mut services = Vec::new();
    for child in &project.nodes[ROOT].children {
        services.extend(convert(&project, *child, input_dir)?);
    }

    let root = SourceNode {
        name: config.project_name.clone(),
        class_name: "DataModel".to_string(),
        file_paths: Vec::new(),
        children: services,
    };

    let text = serde_json::to_string_pretty(&root.into_json())?;
    fs::write(output_file, text)
        .with_context(|| format!("Failed to write {}", output_file.display()))?;

    println!("Wrote sourcemap to {}", output_file.display());
    Ok(())
}

pub fn watch(config: &Config, input_dir: &Path, output_file: &Path) -> Result<()> {
    run(config, input_dir, output_file)?;

    let ignore = output_file.canonicalize().ok();
    let watcher = Watcher::new(input_dir)?;
    println!("Watching {} for changes (Ctrl+C to stop)", input_dir.display());

    while let Some(paths) = watcher.next_batch(Duration::from_millis(200)) {
        let mut relevant = false;
        for path in &paths {
            let resolved = path.canonicalize().ok();
            if ignore.is_none() || resolved != ignore {
                relevant = true;
                break;
            }
        }
        if !relevant {
            continue;
        }

        if let Err(err) = run(config, input_dir, output_file) {
            eprintln!("[Fuse] sourcemap update failed: {err:?}");
        }
    }

    Ok(())
}

fn convert(project: &Project, index: usize, input_dir: &Path) -> Result<Vec<SourceNode>> {
    let node = &project.nodes[index];

    let mut child_sources = Vec::new();
    for child in &node.children {
        child_sources.extend(convert(project, *child, input_dir)?);
    }

    match node.kind {
        NodeKind::Container => Ok(vec![SourceNode {
            name: node.name.clone(),
            class_name: node.class.clone(),
            file_paths: Vec::new(),
            children: child_sources,
        }]),
        NodeKind::Script => {
            let path = node.file.as_ref().unwrap();
            Ok(vec![SourceNode {
                name: node.name.clone(),
                class_name: node.class.clone(),
                file_paths: vec![relative_path(path, input_dir)],
                children: child_sources,
            }])
        }
        NodeKind::Model => {
            let path = node.file.as_ref().unwrap();
            let mut nodes = rbxm_nodes(path, input_dir)?;
            match nodes.first_mut() {
                Some(first) => first.children.extend(child_sources),
                None => nodes = child_sources,
            }
            Ok(nodes)
        }
    }
}

fn rbxm_nodes(path: &Path, input_dir: &Path) -> Result<Vec<SourceNode>> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open model {}", path.display()))?;
    let model = rbx_binary::from_reader(file)
        .with_context(|| format!("Failed to read model {}", path.display()))?;

    let rel = relative_path(path, input_dir);
    let model_root = model.root_ref();
    let mut nodes = Vec::new();

    for child_id in model.get_by_ref(model_root).unwrap().children() {
        let child = model.get_by_ref(*child_id).unwrap();
        if should_skip_sync(child.class.as_str(), child.name.as_str()) {
            continue;
        }

        nodes.push(SourceNode {
            name: child.name.to_string(),
            class_name: child.class.to_string(),
            file_paths: vec![rel.clone()],
            children: Vec::new(),
        });
    }

    Ok(nodes)
}

fn relative_path(path: &Path, root: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let mut parts = Vec::new();
    for component in relative.components() {
        parts.push(component.as_os_str().to_string_lossy().to_string());
    }
    parts.join("/")
}
