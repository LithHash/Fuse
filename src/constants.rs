pub const CONFIG_FILE_NAME: &str = "fuse.json";
pub const HELP_TEXT: &str = r#"Usage:
  fuse <command> [options]

Commands:
  init                     Create a default fuse.json config
  compile                  Compile filesystem files into an rbxl file
  decompile                Decompile an rbxl file into the filesystem
  sourcemap                Generate a Rojo sourcemap for luau-lsp
  merge                    Merge two rbxl files into a new rbxl file

Global Options:
  -h, --help               Show this help message
  -i, --input <path>       Input file or directory (defaults to the current dir)
  -o, --output <path>      Output file or directory path
  -w, --watch              Regenerate on file changes (sourcemap)

Examples:
  fuse init
  fuse compile
  fuse decompile --input test.rbxl
  fuse sourcemap
  fuse sourcemap --watch
  fuse merge --input map.rbxl --input-b codebase.rbxl --output merged.rbxl
"#;

pub const DEFAULT_CONFIG: &str = r#"{
	"project_name": "fuse-project",
	"initStyle": "directoryName",
	"mapping": {
		"src/shared": "ReplicatedStorage",
		"src/server": "ServerScriptService",
		"src/client": "StarterPlayerScripts",
		"src/gui": "StarterGui"
	},
	"customRoutes": {}
}
"#;
