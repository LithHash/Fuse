use crate::{
    config::Config,
    project::{NodeKind, Project, ROOT, build_tree, script_info, should_skip_sync},
};
use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use rbx_dom_weak::{InstanceBuilder, WeakDom, types::Ref};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

struct Loaded {
    scripts: HashMap<PathBuf, String>,
    models: HashMap<PathBuf, WeakDom>,
}

pub fn run(config: &Config, input_dir: &Path, output_file: &Path) -> Result<()> {
    println!("Compiling {}", config.project_name);

    if !input_dir.is_dir() {
        bail!("Compile input must be a directory: {}", input_dir.display());
    }

    let project = build_tree(config, input_dir)?;
    let loaded = load_files(&project)?;

    let mut dom = WeakDom::new(InstanceBuilder::new("DataModel"));
    let root = dom.root_ref();

    let children = project.nodes[ROOT].children.clone();
    for child in children {
        build_node(&project, child, &mut dom, root, &loaded)?;
    }

    let output = fs::File::create(output_file)
        .with_context(|| format!("Failed to create {}", output_file.display()))?;
    let root_children = dom.get_by_ref(root).unwrap().children().to_vec();
    rbx_binary::to_writer(output, &dom, &root_children)
        .with_context(|| format!("Failed to write {}", output_file.display()))?;

    println!(
        "Compiled {} into {}",
        input_dir.display(),
        output_file.display()
    );
    Ok(())
}

fn load_files(project: &Project) -> Result<Loaded> {
    let mut script_paths = Vec::new();
    let mut model_paths = Vec::new();

    for node in &project.nodes {
        if let Some(path) = &node.file {
            match node.kind {
                NodeKind::Script => script_paths.push(path.clone()),
                NodeKind::Model => model_paths.push(path.clone()),
                NodeKind::Container => {}
            }
        }
    }

    let scripts = script_paths
        .into_par_iter()
        .map(|path| {
            let source = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read script {}", path.display()))?;
            Ok((path, source))
        })
        .collect::<Result<HashMap<PathBuf, String>>>()?;

    let models = model_paths
        .into_par_iter()
        .map(|path| {
            let file = fs::File::open(&path)
                .with_context(|| format!("Failed to open model {}", path.display()))?;
            let model = rbx_binary::from_reader(file)
                .with_context(|| format!("Failed to read model {}", path.display()))?;
            Ok((path, model))
        })
        .collect::<Result<HashMap<PathBuf, WeakDom>>>()?;

    Ok(Loaded { scripts, models })
}

fn build_node(
    project: &Project,
    index: usize,
    dom: &mut WeakDom,
    parent: Ref,
    loaded: &Loaded,
) -> Result<()> {
    let node = &project.nodes[index];

    let self_ref = match node.kind {
        NodeKind::Container => dom.insert(
            parent,
            InstanceBuilder::new(node.class.as_str()).with_name(node.name.as_str()),
        ),
        NodeKind::Script => {
            let path = node.file.as_ref().unwrap();
            let source = loaded
                .scripts
                .get(path)
                .with_context(|| format!("Script {} was not loaded", path.display()))?;

            let file_name = path.file_name().unwrap().to_str().unwrap();
            let (_, _, run_context) = script_info(file_name);

            let mut builder = InstanceBuilder::new(node.class.as_str())
                .with_name(node.name.as_str())
                .with_property("Source", source.clone());

            if let Some(run_context) = run_context {
                builder = builder.with_property(
                    "RunContext",
                    rbx_dom_weak::types::Enum::from_u32(run_context),
                );
            }

            dom.insert(parent, builder)
        }
        NodeKind::Model => {
            let path = node.file.as_ref().unwrap();
            let model = loaded
                .models
                .get(path)
                .with_context(|| format!("Model {} was not loaded", path.display()))?;

            match insert_model(model, dom, parent) {
                Some(first) => first,
                None => {
                    eprintln!("[Fuse] {} contains no instances, skipped", path.display());
                    parent
                }
            }
        }
    };

    let children = node.children.clone();
    for child in children {
        build_node(project, child, dom, self_ref, loaded)?;
    }

    Ok(())
}

fn insert_model(model: &WeakDom, dom: &mut WeakDom, parent: Ref) -> Option<Ref> {
    let model_root = model.root_ref();
    let children = model.get_by_ref(model_root).unwrap().children().to_vec();

    let mut first = None;
    for child_id in children {
        let child = model.get_by_ref(child_id).unwrap();
        if should_skip_sync(child.class.as_str(), child.name.as_str()) {
            continue;
        }

        let inserted = clone_tree(model, child_id, dom, parent);
        if first.is_none() {
            first = Some(inserted);
        }
    }

    first
}

fn clone_tree(from: &WeakDom, from_id: Ref, to: &mut WeakDom, parent: Ref) -> Ref {
    let instance = from.get_by_ref(from_id).unwrap();
    let new_id = to.insert(
        parent,
        InstanceBuilder::new(instance.class.as_str())
            .with_name(instance.name.as_str())
            .with_properties(instance.properties.clone()),
    );

    let children = from.get_by_ref(from_id).unwrap().children().to_vec();
    for child_id in children {
        let child = from.get_by_ref(child_id).unwrap();
        if should_skip_sync(child.class.as_str(), child.name.as_str()) {
            continue;
        }

        clone_tree(from, child_id, to, new_id);
    }

    new_id
}
