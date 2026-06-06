use crate::config::Config;
use anyhow::{Context, Result};
use rbx_dom_weak::{InstanceBuilder, Ustr, WeakDom, types::Variant};
use std::{fs, path::Path};

pub fn run(config: &Config, input_file: &Path, output_dir: &Path) -> Result<()> {
    println!("Decompiling {}", config.project_name);

    let input = fs::File::open(input_file)
        .with_context(|| format!("Failed to open {}", input_file.display()))?;
    let dom = rbx_binary::from_reader(input)
        .with_context(|| format!("Failed to read {}", input_file.display()))?;

    fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create {}", output_dir.display()))?;

    let root = dom.get_by_ref(dom.root_ref()).unwrap();
    for service_id in root.children().to_vec() {
        let service = dom.get_by_ref(service_id).unwrap();
        let Some(service_dir) = config.path_for_service(output_dir, service.name.as_str()) else {
            continue;
        };

        fs::create_dir_all(&service_dir)
            .with_context(|| format!("Failed to create {}", service_dir.display()))?;

        for child_id in service.children().to_vec() {
            write_instance(&dom, child_id, &service_dir)?;
        }
    }

    println!(
        "Decompiled {} into {}",
        input_file.display(),
        output_dir.display()
    );
    Ok(())
}

fn write_instance(dom: &WeakDom, id: rbx_dom_weak::types::Ref, parent_dir: &Path) -> Result<()> {
    let instance = dom.get_by_ref(id).unwrap();

    if should_skip_sync(instance.class.as_str(), instance.name.as_str()) {
        return Ok(());
    }

    let children = instance.children().to_vec();
    let file_name = safe_file_name(instance.name.as_str());

    if is_script(instance.class.as_str()) {
        if children.is_empty() {
            write_script(instance, parent_dir)?;
            return Ok(());
        }

        let dir = parent_dir.join(&file_name);
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
        write_script(instance, &dir)?;

        for child_id in children {
            write_instance(dom, child_id, &dir)?;
        }

        return Ok(());
    }

    if children.is_empty() {
        write_rbxm(dom, id, &parent_dir.join(format!("{file_name}.rbxm")))?;
        return Ok(());
    }

    let dir = parent_dir.join(&file_name);
    fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;

    write_rbxm(dom, id, &dir.join(format!("{file_name}.rbxm")))?;

    for child_id in children {
        write_instance(dom, child_id, &dir)?;
    }

    Ok(())
}

fn write_script(instance: &rbx_dom_weak::Instance, parent_dir: &Path) -> Result<()> {
    let safe_name = safe_file_name(instance.name.as_str());
    let file_name = match script_kind(instance) {
        ScriptKind::Server => format!("{safe_name}.server.luau"),
        ScriptKind::Client => format!("{safe_name}.client.luau"),
        ScriptKind::Module => format!("{safe_name}.luau"),
    };

    let source = match instance.properties.get(&Ustr::from("Source")) {
        Some(Variant::String(source)) => source.as_str(),
        _ => "",
    };

    fs::write(parent_dir.join(file_name), source)
        .with_context(|| format!("Failed to write script {}", instance.name))?;

    Ok(())
}

fn write_rbxm(dom: &WeakDom, id: rbx_dom_weak::types::Ref, path: &Path) -> Result<()> {
    let mut model = WeakDom::new(InstanceBuilder::new("Folder"));
    let model_root = model.root_ref();

    clone_without_children(dom, id, &mut model, model_root)?;

    let file = fs::File::create(path)
        .with_context(|| format!("Failed to create model {}", path.display()))?;
    let children = model.get_by_ref(model_root).unwrap().children().to_vec();
    rbx_binary::to_writer(file, &model, &children)
        .with_context(|| format!("Failed to write model {}", path.display()))?;

    Ok(())
}

fn clone_without_children(
    from_dom: &WeakDom,
    from_id: rbx_dom_weak::types::Ref,
    to_dom: &mut WeakDom,
    parent_id: rbx_dom_weak::types::Ref,
) -> Result<rbx_dom_weak::types::Ref> {
    let instance = from_dom.get_by_ref(from_id).unwrap();

    let new_id = to_dom.insert(
        parent_id,
        InstanceBuilder::new(instance.class.as_str())
            .with_name(instance.name.as_str())
            .with_properties(instance.properties.clone()),
    );

    Ok(new_id)
}

fn is_script(class_name: &str) -> bool {
    matches!(class_name, "Script" | "LocalScript" | "ModuleScript")
}

enum ScriptKind {
    Server,
    Client,
    Module,
}

fn script_kind(instance: &rbx_dom_weak::Instance) -> ScriptKind {
    match instance.class.as_str() {
        "ModuleScript" => ScriptKind::Module,
        "LocalScript" => ScriptKind::Client,
        "Script" => {
            let run_context = instance.properties.get(&Ustr::from("RunContext"));
            if let Some(Variant::Enum(value)) = run_context {
                if value.to_u32() == 2 {
                    return ScriptKind::Client;
                }
            }

            ScriptKind::Server
        }
        _ => ScriptKind::Module,
    }
}

fn safe_file_name(name: &str) -> String {
    let mut output = String::new();

    for character in name.chars() {
        let safe_character = match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => character,
        };

        output.push(safe_character);
    }

    let output = output.trim().trim_end_matches('.');
    if output.is_empty() {
        "Instance".to_string()
    } else {
        output.to_string()
    }
}

fn should_skip_sync(class_name: &str, name: &str) -> bool {
    matches!(class_name, "Terrain" | "Camera") || matches!(name, "Terrain" | "Camera")
}
