use crate::config::Config;
use anyhow::{Context, Result, bail};
use rbx_dom_weak::{InstanceBuilder, WeakDom};
use std::{fs, path::Path};

pub fn run(config: &Config, input_dir: &Path, output_file: &Path) -> Result<()> {
    println!("Compiling {}", config.project_name);

    if !input_dir.is_dir() {
        bail!("Compile input must be a directory: {}", input_dir.display());
    }

    let mut dom = WeakDom::new(InstanceBuilder::new("DataModel"));
    let root_id = dom.root_ref();

    for item in &config.mapping {
        let source_dir = input_dir.join(&item.file_path);
        if !source_dir.exists() {
            continue;
        }

        let service_id = dom.insert(
            root_id,
            InstanceBuilder::new(item.service_name.as_str()).with_name(item.service_name.as_str()),
        );

        compile_dir(&source_dir, &mut dom, service_id)?;
    }

    let output = fs::File::create(output_file)
        .with_context(|| format!("Failed to create {}", output_file.display()))?;

    let root_children = dom.get_by_ref(root_id).unwrap().children().to_vec();
    rbx_binary::to_writer(output, &dom, &root_children)
        .with_context(|| format!("Failed to write {}", output_file.display()))?;

    println!(
        "Compiled {} into {}",
        input_dir.display(),
        output_file.display()
    );
    Ok(())
}

fn compile_dir(dir: &Path, dom: &mut WeakDom, parent_id: rbx_dom_weak::types::Ref) -> Result<()> {
    let dir_name = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("src");
    let backing_file = dir.join(format!("{dir_name}.rbxm"));
    let backing_script = find_backing_script(dir, dir_name);

    let actual_parent = if backing_file.exists() {
        insert_rbxm(dom, parent_id, &backing_file)?
    } else if let Some(path) = &backing_script {
        insert_script(dom, parent_id, path)?
    } else {
        parent_id
    };

    for entry in fs::read_dir(dir).with_context(|| format!("Failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();

        if path == backing_file {
            continue;
        }

        if let Some(backing_script) = &backing_script {
            if path == *backing_script {
                continue;
            }
        }

        if path.is_dir() {
            compile_dir(&path, dom, actual_parent)?;
            continue;
        }

        if is_luau_file(&path) {
            insert_script(dom, actual_parent, &path)?;
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) == Some("rbxm") {
            insert_rbxm(dom, actual_parent, &path)?;
        }
    }

    Ok(())
}

fn insert_script(
    dom: &mut WeakDom,
    parent_id: rbx_dom_weak::types::Ref,
    path: &Path,
) -> Result<rbx_dom_weak::types::Ref> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Script.luau");
    let source = fs::read_to_string(path)
        .with_context(|| format!("Failed to read script {}", path.display()))?;

    let (name, class_name, run_context) = script_info(file_name);
    let mut builder = InstanceBuilder::new(class_name)
        .with_name(name)
        .with_property("Source", source);

    if let Some(run_context) = run_context {
        builder = builder.with_property(
            "RunContext",
            rbx_dom_weak::types::Enum::from_u32(run_context),
        );
    }

    let id = dom.insert(parent_id, builder);
    Ok(id)
}

fn insert_rbxm(
    dom: &mut WeakDom,
    parent_id: rbx_dom_weak::types::Ref,
    path: &Path,
) -> Result<rbx_dom_weak::types::Ref> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open model {}", path.display()))?;
    let model = rbx_binary::from_reader(file)
        .with_context(|| format!("Failed to read model {}", path.display()))?;

    let model_root = model.root_ref();
    let children = model.get_by_ref(model_root).unwrap().children().to_vec();
    let mut first_inserted = None;

    for child_id in children {
        let child = model.get_by_ref(child_id).unwrap();
        if should_skip_sync(child.class.as_str(), child.name.as_str()) {
            continue;
        }

        let inserted = clone_instance_tree(&model, child_id, dom, parent_id, child.name.as_str())?;
        first_inserted.get_or_insert(inserted);
    }

    first_inserted
        .with_context(|| format!("Model {} did not contain any instances", path.display()))
}

fn clone_instance_tree(
    from_dom: &WeakDom,
    from_id: rbx_dom_weak::types::Ref,
    to_dom: &mut WeakDom,
    parent_id: rbx_dom_weak::types::Ref,
    name: &str,
) -> Result<rbx_dom_weak::types::Ref> {
    let instance = from_dom.get_by_ref(from_id).unwrap();
    let new_id = to_dom.insert(
        parent_id,
        InstanceBuilder::new(instance.class.as_str())
            .with_name(name)
            .with_properties(instance.properties.clone()),
    );

    for child_id in from_dom.get_by_ref(from_id).unwrap().children().to_vec() {
        let child = from_dom.get_by_ref(child_id).unwrap();
        if should_skip_sync(child.class.as_str(), child.name.as_str()) {
            continue;
        }

        clone_instance_tree(from_dom, child_id, to_dom, new_id, child.name.as_str())?;
    }

    Ok(new_id)
}

fn is_luau_file(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str()) == Some("luau")
}

fn find_backing_script(dir: &Path, dir_name: &str) -> Option<std::path::PathBuf> {
    let server_script = dir.join(format!("{dir_name}.server.luau"));
    if server_script.exists() {
        return Some(server_script);
    }

    let client_script = dir.join(format!("{dir_name}.client.luau"));
    if client_script.exists() {
        return Some(client_script);
    }

    let module_script = dir.join(format!("{dir_name}.luau"));
    if module_script.exists() {
        return Some(module_script);
    }

    None
}

fn script_info(file_name: &str) -> (&str, &str, Option<u32>) {
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

fn should_skip_sync(class_name: &str, name: &str) -> bool {
    matches!(class_name, "Terrain" | "Camera") || matches!(name, "Terrain" | "Camera")
}
