use std::{fs, path::Path};

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::constants::CONFIG_FILE_NAME;

pub struct Config {
    pub project_name: String,
    pub mapping: Vec<Mapping>,
}

pub struct Mapping {
    pub file_path: String,
    pub service_name: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        let text = fs::read_to_string(CONFIG_FILE_NAME)
            .with_context(|| format!("Missing {CONFIG_FILE_NAME}. Run `fuse init` first."))?;

        let json: Value = serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse {CONFIG_FILE_NAME}"))?;

        let project_name = read_string(&json, "project_name")?;
        let mapping = read_mapping(&json)?;

        Ok(Config {
            project_name,
            mapping,
        })
    }

    pub fn path_for_service(&self, root: &Path, service_name: &str) -> Option<std::path::PathBuf> {
        for item in &self.mapping {
            if item.service_name == service_name {
                return Some(root.join(&item.file_path));
            }
        }

        None
    }
}

fn read_string(json: &Value, key: &str) -> Result<String> {
    let value = &json[key];
    if let Value::String(text) = value {
        return Ok(text.clone());
    }

    bail!("{CONFIG_FILE_NAME} needs a string named {key}")
}

fn read_mapping(json: &Value) -> Result<Vec<Mapping>> {
    let value = &json["mapping"];
    let Value::Object(object) = value else {
        bail!("{CONFIG_FILE_NAME} needs a mapping object");
    };

    let mut mapping = Vec::new();

    for (file_path, service_value) in object {
        let Value::String(service_name) = service_value else {
            bail!("mapping value for {file_path} must be a string");
        };

        mapping.push(Mapping {
            file_path: file_path.clone(),
            service_name: service_name.clone(),
        });
    }

    Ok(mapping)
}
