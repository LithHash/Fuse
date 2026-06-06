# Fuse

Fuse is a small CLI for moving Roblox projects between `.rbxl` files and a filesystem layout.

*live script sync is planned for the future*

It can:
- decompile an `.rbxl` into folders, scripts, and `.rbxm` files
- compile that filesystem layout back into an `.rbxl`
- merge two `.rbxl` files into one place file

## Install

With Rokit:

```bash
rokit add LithHash/fuse
```

Or add it to `rokit.toml` manually:

```toml
[tools]
fuse = "LithHash/fuse@0.1.0"
```

Then install project tools:

```bash
rokit install
```

## Build

```bash
cargo build --release
```

## Usage

### Initialize

Create a default `fuse.json` config:

```bash
fuse init
```

### Decompile

```bash
fuse decompile --input game.rbxl --output .
```

### Compile

```bash
fuse compile --input . --output game.rbxl
```

### Merge

```bash
fuse merge --input map.rbxl --input-b codebase.rbxl --output merged.rbxl
```

This is useful when keeping map work and code work in separate place files.

## Configuration

Fuse uses `fuse.json` to decide which folders map to which Roblox services.

```json
{
	"project_name": "fuse-project",
	"mapping": {
		"src/shared": "ReplicatedStorage",
		"src/server": "ServerScriptService",
		"src/client": "StarterPlayerScripts",
		"src/gui": "StarterGui"
	}
}
```

## File layout

Example project:

```text
.
├── fuse.json
└── src
	├── workspace
	├── shared
	├── server
	├── client
	└── gui
```

## Script files

Fuse uses file suffixes to choose script types:

| File suffix | Roblox instance |
|-------------|-----------------|
| `.server.luau` | `Script` |
| `.client.luau` | client `Script` |
| `.luau` | `ModuleScript` |

Non-script instances are stored as `.rbxm` files.

## License

MIT. See [LICENSE](LICENSE).
