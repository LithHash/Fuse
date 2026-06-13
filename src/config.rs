use std::fs;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::{
    constants::CONFIG_FILE_NAME,
    routes::{glob_match, parse_roblox_path},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum InitStyle {
    DirectoryName,
    InitFile,
    Sibling,
}

pub struct Config {
    pub project_name: String,
    pub input_mappings: Vec<InputMapping>,
    pub output_mappings: Vec<OutputMapping>,
    pub routes: Vec<Route>,
    pub init_style: InitStyle,
}

pub struct InputMapping {
    pub fs_path: String,
    pub roblox_path: Vec<String>,
}

pub struct OutputMapping {
    pub roblox_path: Vec<String>,
    pub fs_path: String,
}

pub struct Route {
    pub pattern: String,
    pub roblox_path: Vec<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let text = fs::read_to_string(CONFIG_FILE_NAME)
            .with_context(|| format!("Missing {CONFIG_FILE_NAME}. Run `fuse init` first."))?;

        Self::from_json(&text)
    }

    pub fn from_json(text: &str) -> Result<Self> {
        let json: Value = serde_json::from_str(text)
            .with_context(|| format!("Failed to parse {CONFIG_FILE_NAME}"))?;

        let project_name = read_string(&json, "project_name")?;

        let shared = object_entries(&json, "mapping")?;
        let input_only = object_entries(&json, "inputMapping")?;
        let output_only = object_entries(&json, "outputMapping")?;

        if shared.is_none() && input_only.is_none() && output_only.is_none() {
            bail!("{CONFIG_FILE_NAME} needs a mapping, inputMapping, or outputMapping object");
        }

        let mut input_mappings: Vec<InputMapping> = Vec::new();
        for (fs_path, target) in shared.clone().unwrap_or_default() {
            let roblox_path = parse_roblox_path(&target);
            if roblox_path.is_empty() {
                bail!("mapping for {fs_path} has an empty Roblox path");
            }

            input_mappings.push(InputMapping {
                fs_path,
                roblox_path,
            });
        }

        for (fs_path, target) in input_only.unwrap_or_default() {
            let roblox_path = parse_roblox_path(&target);
            if roblox_path.is_empty() {
                bail!("inputMapping for {fs_path} has an empty Roblox path");
            }

            let mut replaced = false;
            for existing in &mut input_mappings {
                if existing.fs_path == fs_path {
                    existing.roblox_path = roblox_path.clone();
                    replaced = true;
                    break;
                }
            }

            if !replaced {
                input_mappings.push(InputMapping {
                    fs_path,
                    roblox_path,
                });
            }
        }

        let mut output_mappings: Vec<OutputMapping> = Vec::new();
        for (target, fs_path) in output_only.unwrap_or_default() {
            let roblox_path = parse_roblox_path(&target);
            if roblox_path.is_empty() {
                bail!("outputMapping key {target:?} is not a valid Roblox path");
            }

            output_mappings.push(OutputMapping {
                roblox_path,
                fs_path,
            });
        }

        for (fs_path, target) in shared.unwrap_or_default() {
            let roblox_path = parse_roblox_path(&target);

            let mut taken = false;
            for existing in &output_mappings {
                if existing.roblox_path == roblox_path {
                    taken = true;
                    break;
                }
            }

            if !taken {
                output_mappings.push(OutputMapping {
                    roblox_path,
                    fs_path,
                });
            }
        }

        let mut routes = Vec::new();
        if let Some(entries) = object_entries(&json, "customRoutes")? {
            for (pattern, target) in entries {
                let roblox_path = parse_roblox_path(&target);
                if roblox_path.is_empty() {
                    bail!("customRoutes entry {pattern} has an empty Roblox path");
                }

                routes.push(Route {
                    pattern,
                    roblox_path,
                });
            }
        }

        let init_style = match json.get("initStyle") {
            None => InitStyle::DirectoryName,
            Some(Value::String(text)) => match text.as_str() {
                "directoryName" => InitStyle::DirectoryName,
                "init" => InitStyle::InitFile,
                "sibling" => InitStyle::Sibling,
                other => bail!("Unknown initStyle {other:?}, expected directoryName, init, or sibling"),
            },
            Some(_) => bail!("initStyle must be a string"),
        };

        Ok(Config {
            project_name,
            input_mappings,
            output_mappings,
            routes,
            init_style,
        })
    }

    pub fn route_for(&self, name: &str) -> Option<&[String]> {
        for route in &self.routes {
            if glob_match(&route.pattern, name) {
                return Some(&route.roblox_path);
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

fn object_entries(json: &Value, key: &str) -> Result<Option<Vec<(String, String)>>> {
    let Some(value) = json.get(key) else {
        return Ok(None);
    };

    let Value::Object(object) = value else {
        bail!("{key} in {CONFIG_FILE_NAME} must be an object");
    };

    let mut entries = Vec::new();
    for (entry_key, entry_value) in object {
        let Value::String(text) = entry_value else {
            bail!("{key} value for {entry_key} must be a string");
        };

        entries.push((entry_key.clone(), text.clone()));
    }

    Ok(Some(entries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_works_in_both_directions() {
        let config = Config::from_json(
            r#"{
                "project_name": "shared",
                "mapping": {
                    "src/server": "ServerScriptService",
                    "src/client": "StarterPlayerScripts"
                }
            }"#,
        )
        .unwrap();

        assert_eq!(config.input_mappings.len(), 2);
        assert_eq!(config.input_mappings[0].fs_path, "src/server");
        assert_eq!(config.input_mappings[0].roblox_path, ["ServerScriptService"]);
        assert_eq!(
            config.input_mappings[1].roblox_path,
            ["StarterPlayer", "StarterPlayerScripts"]
        );

        assert_eq!(config.output_mappings.len(), 2);
        assert_eq!(config.output_mappings[0].roblox_path, ["ServerScriptService"]);
        assert_eq!(config.output_mappings[0].fs_path, "src/server");
    }

    #[test]
    fn input_and_output_override_the_shared_mapping() {
        let config = Config::from_json(
            r#"{
                "project_name": "granular",
                "mapping": { "src": "ReplicatedStorage/Source" },
                "inputMapping": { "vendor": "ReplicatedStorage/Vendor" },
                "outputMapping": { "ReplicatedStorage/Source": "code" }
            }"#,
        )
        .unwrap();

        assert_eq!(config.input_mappings.len(), 2);
        assert_eq!(config.input_mappings[1].fs_path, "vendor");

        assert_eq!(config.output_mappings.len(), 1);
        assert_eq!(config.output_mappings[0].fs_path, "code");
    }

    #[test]
    fn init_style_parses() {
        let config = Config::from_json(
            r#"{
                "project_name": "styles",
                "mapping": { "src": "ReplicatedStorage" },
                "initStyle": "init"
            }"#,
        )
        .unwrap();
        assert!(config.init_style == InitStyle::InitFile);

        let bad = Config::from_json(
            r#"{
                "project_name": "styles",
                "mapping": { "src": "ReplicatedStorage" },
                "initStyle": "banana"
            }"#,
        );
        assert!(bad.is_err());
    }

    #[test]
    fn routes_match_globs() {
        let config = Config::from_json(
            r#"{
                "project_name": "routes",
                "mapping": { "src": "ReplicatedStorage" },
                "customRoutes": {
                    "server": "ServerScriptService",
                    "*.client.luau": "StarterPlayerScripts"
                }
            }"#,
        )
        .unwrap();

        assert_eq!(config.route_for("server").unwrap(), ["ServerScriptService"]);
        assert_eq!(
            config.route_for("Camera.client.luau").unwrap(),
            ["StarterPlayer", "StarterPlayerScripts"]
        );
        assert!(config.route_for("Items.luau").is_none());
    }

    #[test]
    fn missing_mapping_is_an_error() {
        assert!(Config::from_json(r#"{ "project_name": "broken" }"#).is_err());
    }
}
