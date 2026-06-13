use crate::{
    config::{Config, InitStyle},
    project::{is_script_class, should_skip_sync},
};
use anyhow::{Context, Result};
use rayon::prelude::*;
use rbx_dom_weak::{InstanceBuilder, Ustr, WeakDom, types::Ref, types::Variant};
use std::{
    collections::HashSet,
    fs,
    io::BufReader,
    path::Path,
};

struct Plan<'a> {
    dom: &'a WeakDom,
    claimed: HashSet<Ref>,
    has_script: HashSet<Ref>,
    style: InitStyle,
}

pub fn run(config: &Config, input_file: &Path, output_dir: &Path) -> Result<()> {
    println!("Decompiling {}", config.project_name);

    let input = fs::File::open(input_file)
        .with_context(|| format!("Failed to open {}", input_file.display()))?;
    let dom = rbx_binary::from_reader(BufReader::new(input))
        .with_context(|| format!("Failed to read {}", input_file.display()))?;

    fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create {}", output_dir.display()))?;

    let mut targets = Vec::new();
    for mapping in &config.output_mappings {
        if let Some(id) = find_by_path(&dom, &mapping.roblox_path) {
            targets.push((id, output_dir.join(&mapping.fs_path)));
        }
    }

    let mut claimed = HashSet::new();
    for (id, _) in &targets {
        claimed.insert(*id);
    }

    let mut has_script = HashSet::new();
    for (id, _) in &targets {
        let children = dom.get_by_ref(*id).unwrap().children().to_vec();
        for child in children {
            mark_scripts(&dom, child, &claimed, &mut has_script);
        }
    }

    let plan = Plan {
        dom: &dom,
        claimed,
        has_script,
        style: config.init_style,
    };

    targets.par_iter().try_for_each(|(id, dir)| -> Result<()> {
        fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;

        let children = plan.dom.get_by_ref(*id).unwrap().children().to_vec();
        let assigned = assign_names(&plan, &children, None);
        assigned
            .par_iter()
            .try_for_each(|(child_id, name)| write_instance(&plan, *child_id, dir, name))
    })?;

    println!(
        "Decompiled {} into {}",
        input_file.display(),
        output_dir.display()
    );
    Ok(())
}

fn find_by_path(dom: &WeakDom, path: &[String]) -> Option<Ref> {
    let mut current = dom.root_ref();

    for segment in path {
        let instance = dom.get_by_ref(current)?;
        let mut next = None;
        for child_id in instance.children() {
            let child = dom.get_by_ref(*child_id)?;
            if child.name == segment.as_str() {
                next = Some(*child_id);
                break;
            }
        }
        current = next?;
    }

    Some(current)
}

fn mark_scripts(dom: &WeakDom, id: Ref, claimed: &HashSet<Ref>, set: &mut HashSet<Ref>) -> bool {
    let instance = dom.get_by_ref(id).unwrap();
    if should_skip_sync(instance.class.as_str(), instance.name.as_str()) {
        return false;
    }

    let mut has = is_script_class(instance.class.as_str());
    for child_id in instance.children() {
        if claimed.contains(child_id) {
            continue;
        }
        if mark_scripts(dom, *child_id, claimed, set) {
            has = true;
        }
    }

    if has {
        set.insert(id);
    }
    has
}

fn assign_names(plan: &Plan, children: &[Ref], reserved: Option<&str>) -> Vec<(Ref, String)> {
    let mut used = HashSet::new();
    if let Some(reserved) = reserved {
        used.insert(reserved.to_lowercase());
    }

    let mut assigned = Vec::new();
    for child_id in children {
        if plan.claimed.contains(child_id) {
            continue;
        }

        let child = plan.dom.get_by_ref(*child_id).unwrap();
        if should_skip_sync(child.class.as_str(), child.name.as_str()) {
            continue;
        }

        let base = safe_file_name(child.name.as_str());
        let mut name = base.clone();
        let mut counter = 1;
        while !used.insert(name.to_lowercase()) {
            counter += 1;
            name = format!("{base}_{counter}");
        }

        assigned.push((*child_id, name));
    }

    assigned
}

fn write_instance(plan: &Plan, id: Ref, parent_dir: &Path, name: &str) -> Result<()> {
    let instance = plan.dom.get_by_ref(id).unwrap();
    let children = instance.children();

    if is_script_class(instance.class.as_str()) {
        if children.is_empty() {
            write_script(plan.style, instance, parent_dir, name, false)?;
            return Ok(());
        }

        let dir = parent_dir.join(name);
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;

        let script_dir = backing_dir(plan.style, parent_dir, &dir);
        write_script(plan.style, instance, script_dir, name, true)?;

        write_children(plan, children, &dir, name)?;
        return Ok(());
    }

    if !plan.has_script.contains(&id) {
        write_rbxm(plan, id, &parent_dir.join(format!("{name}.rbxm")), true)?;
        return Ok(());
    }

    let dir = parent_dir.join(name);
    fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;

    let model_dir = backing_dir(plan.style, parent_dir, &dir);
    let model_name = backing_name(plan.style, name, "rbxm");
    write_rbxm(plan, id, &model_dir.join(model_name), false)?;

    write_children(plan, children, &dir, name)?;
    Ok(())
}

fn write_children(plan: &Plan, children: &[Ref], dir: &Path, dir_name: &str) -> Result<()> {
    let reserved = match plan.style {
        InitStyle::DirectoryName => Some(dir_name.to_string()),
        InitStyle::InitFile => Some("init".to_string()),
        InitStyle::Sibling => None,
    };

    let assigned = assign_names(plan, children, reserved.as_deref());
    assigned
        .par_iter()
        .try_for_each(|(child_id, name)| write_instance(plan, *child_id, dir, name))
}

fn backing_dir<'a>(style: InitStyle, parent_dir: &'a Path, dir: &'a Path) -> &'a Path {
    match style {
        InitStyle::Sibling => parent_dir,
        _ => dir,
    }
}

fn backing_name(style: InitStyle, name: &str, extension: &str) -> String {
    match style {
        InitStyle::InitFile => format!("init.{extension}"),
        _ => format!("{name}.{extension}"),
    }
}

fn write_script(
    style: InitStyle,
    instance: &rbx_dom_weak::Instance,
    dir: &Path,
    name: &str,
    is_backing: bool,
) -> Result<()> {
    let suffix = match script_kind(instance) {
        ScriptKind::Server => "server.luau",
        ScriptKind::Client => "client.luau",
        ScriptKind::Module => "luau",
    };

    let stem = if is_backing && style == InitStyle::InitFile {
        "init"
    } else {
        name
    };

    let source = match instance.properties.get(&Ustr::from("Source")) {
        Some(Variant::String(source)) => source.as_str(),
        _ => "",
    };

    fs::write(dir.join(format!("{stem}.{suffix}")), source)
        .with_context(|| format!("Failed to write script {}", instance.name))?;

    Ok(())
}

fn write_rbxm(plan: &Plan, id: Ref, path: &Path, deep: bool) -> Result<()> {
    let mut model = WeakDom::new(InstanceBuilder::new("Folder"));
    let model_root = model.root_ref();

    clone_into(plan, id, &mut model, model_root, deep);

    let children = model.get_by_ref(model_root).unwrap().children().to_vec();
    let mut buffer = Vec::new();
    rbx_binary::to_writer(&mut buffer, &model, &children)
        .with_context(|| format!("Failed to serialize model {}", path.display()))?;
    fs::write(path, &buffer)
        .with_context(|| format!("Failed to write model {}", path.display()))?;

    Ok(())
}

fn clone_into(plan: &Plan, from_id: Ref, to: &mut WeakDom, parent: Ref, deep: bool) {
    let instance = plan.dom.get_by_ref(from_id).unwrap();

    let new_id = to.insert(
        parent,
        InstanceBuilder::new(instance.class.as_str())
            .with_name(instance.name.as_str())
            .with_properties(instance.properties.clone()),
    );

    if !deep {
        return;
    }

    let children = instance.children().to_vec();
    for child_id in children {
        if plan.claimed.contains(&child_id) {
            continue;
        }

        let child = plan.dom.get_by_ref(child_id).unwrap();
        if should_skip_sync(child.class.as_str(), child.name.as_str()) {
            continue;
        }

        clone_into(plan, child_id, to, new_id, true);
    }
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
            if let Some(Variant::Enum(value)) = run_context
                && value.to_u32() == 2
            {
                return ScriptKind::Client;
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
