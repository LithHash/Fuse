use anyhow::{Context, Result, bail};
use rbx_binary::{from_reader, to_writer};
use rbx_dom_weak::{InstanceBuilder, WeakDom};
use std::fs;
use std::path::Path;

pub fn run(input_a: &Path, input_b: &Path, output_file: &Path) -> Result<()> {
    if !input_a.is_file() {
        bail!("First merge input must be a file: {}", input_a.display());
    }

    if !input_b.is_file() {
        bail!("Second merge input must be a file: {}", input_b.display());
    }

    let dom_a = load_dom(input_a)?;
    let dom_b = load_dom(input_b)?;
    let mut merged = WeakDom::new(InstanceBuilder::new("DataModel"));
    let merged_root = merged.root_ref();

    merge_children(&dom_a, dom_a.root_ref(), &mut merged, merged_root)?;
    merge_children(&dom_b, dom_b.root_ref(), &mut merged, merged_root)?;

    let output = fs::File::create(output_file)
        .with_context(|| format!("Failed to create {}", output_file.display()))?;
    let root_children = merged.get_by_ref(merged_root).unwrap().children().to_vec();
    to_writer(output, &merged, &root_children)
        .with_context(|| format!("Failed to write {}", output_file.display()))?;

    println!(
        "Merged {} and {} into {}",
        input_a.display(),
        input_b.display(),
        output_file.display()
    );
    Ok(())
}

fn load_dom(path: &Path) -> Result<WeakDom> {
    let file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    from_reader(file).with_context(|| format!("Failed to read {}", path.display()))
}

fn merge_children(
    source: &WeakDom,
    source_id: rbx_dom_weak::types::Ref,
    target: &mut WeakDom,
    parent_id: rbx_dom_weak::types::Ref,
) -> Result<()> {
    let source_instance = source.get_by_ref(source_id).unwrap();

    for child_id in source_instance.children().to_vec() {
        let child = source.get_by_ref(child_id).unwrap();

        if should_merge_singleton(child.class.as_str(), child.name.as_str())
            && let Some(existing_id) =
                find_singleton_child(target, parent_id, child.class.as_str(), child.name.as_str())
        {
            merge_children(source, child_id, target, existing_id)?;
            continue;
        }

        clone_instance_tree(source, child_id, target, parent_id)?;
    }

    Ok(())
}

fn clone_instance_tree(
    source: &WeakDom,
    source_id: rbx_dom_weak::types::Ref,
    target: &mut WeakDom,
    parent_id: rbx_dom_weak::types::Ref,
) -> Result<rbx_dom_weak::types::Ref> {
    let instance = source.get_by_ref(source_id).unwrap();
    let new_id = target.insert(
        parent_id,
        InstanceBuilder::new(instance.class.as_str())
            .with_name(instance.name.as_str())
            .with_properties(instance.properties.clone()),
    );

    for child_id in instance.children().to_vec() {
        clone_instance_tree(source, child_id, target, new_id)?;
    }

    Ok(new_id)
}

fn find_singleton_child(
    dom: &WeakDom,
    parent_id: rbx_dom_weak::types::Ref,
    class_name: &str,
    name: &str,
) -> Option<rbx_dom_weak::types::Ref> {
    let parent = dom.get_by_ref(parent_id).unwrap();

    for child_id in parent.children() {
        let child = dom.get_by_ref(*child_id).unwrap();
        if child.class == class_name && child.name == name {
            return Some(*child_id);
        }

        if is_engine_singleton(class_name) && child.class == class_name {
            return Some(*child_id);
        }
    }

    None
}

fn should_merge_singleton(class_name: &str, name: &str) -> bool {
    is_service(class_name)
        || is_engine_singleton(class_name)
        || matches!(name, "Terrain" | "Camera")
}

fn is_engine_singleton(class_name: &str) -> bool {
    matches!(class_name, "Terrain" | "Camera")
}

fn is_service(class_name: &str) -> bool {
    matches!(
        class_name,
        "Workspace"
            | "ReplicatedStorage"
            | "ServerScriptService"
            | "ServerStorage"
            | "StarterGui"
            | "StarterPack"
            | "StarterPlayer"
            | "Lighting"
            | "SoundService"
            | "Teams"
            | "Chat"
            | "TextChatService"
    )
}
