pub const CONFIG_FILE_NAME: &str = "fuse.json";
pub const HELP_TEXT: &str = r#"Usage:
  fuse <command> [options]

Commands:
  init                     Create a default fuse.json config
  compile                  Compile filesystem files into an rbxl file
  decompile                Decompile an rbxl file into the filesystem
  merge                    Merge two rbxl files into a new rbxl file

Global Options:
  -h, --help               Show this help message
  --output <path>          Output file or directory path

Examples:
  fuse init
  fuse compile --input . --output test.rbxl
  fuse decompile --input test.rbxl --output .
  fuse merge --input map.rbxl --input-b codebase.rbxl --output merged.rbxl
"#;

pub const DEFAULT_CONFIG: &str = r#"{
	"project_name": "fuse-project",
	"mapping": {
		"src/shared": "ReplicatedStorage",
		"src/server": "ServerScriptService",
		"src/client": "StarterPlayerScripts",
		"src/gui": "StarterGui"
	}
}
"#;
