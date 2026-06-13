# Fuse

Fuse turns a Roblox `.rbxl`/`.rbxm` into a folder tree, then builds it back again.

Use it to:

- decompile places into `.luau` scripts and `.rbxm` models
- compile folders back into an `.rbxl`
- merge two `.rbxl` files
- generate a Rojo-compatible sourcemap for luau-lsp

Live sync is not built yet.

## Install

With [Rokit](https://github.com/rojo-rbx/rokit):

```bash
rokit add LithHash/fuse
```

Or from source:

```bash
cargo build --release
```

## Usage

```bash
fuse init
fuse decompile --input game.rbxl
fuse compile
fuse merge --input map.rbxl --input-b code.rbxl --output out.rbxl
fuse sourcemap
fuse sourcemap --watch
```

Defaults:

- `compile` reads `.` and writes `<project_name>.rbxl`
- `decompile` writes to `.`
- `sourcemap` reads `.` and writes `sourcemap.json`
- `merge` requires `--input`, `--input-b`, and `--output`

## Configuration

`fuse.json` maps folders to Roblox paths:

```json
{
	"project_name": "my-game",
	"initStyle": "directoryName",
	"mapping": {
		"src/client": "StarterPlayerScripts",
		"src/server": "ServerScriptService",
		"src/shared": "ReplicatedStorage/Shared",
		"src/ui": "StarterGui"
	},
	"customRoutes": {}
}
```

`mapping` works both ways. Compile sends folders to those Roblox paths. Decompile writes those Roblox paths back to the same folders.

Use `inputMapping` or `outputMapping` when one direction needs different paths:

```json
{
	"mapping": {
		"src/client": "StarterPlayerScripts",
		"src/server": "ServerScriptService",
		"src/shared": "ReplicatedStorage/Shared"
	},
	"inputMapping": {
		"packages": "ReplicatedStorage/Packages"
	},
	"outputMapping": {
		"ReplicatedStorage/Shared": "src/shared",
		"ReplicatedStorage/Packages": "packages"
	}
}
```

Nested Roblox paths are allowed. `StarterPlayerScripts` and `StarterCharacterScripts` are shorthand for paths under `StarterPlayer`.

If `outputMapping` entries overlap, the most specific path wins.

## Init style

`initStyle` controls the file used for a folder's own instance:

- `directoryName`: `Widget/Widget.luau`
- `init`: `Widget/init.luau`
- `sibling`: `Widget.luau` next to `Widget/`

`.rbxm` files follow the same rule.

## Custom routes

`customRoutes` redirects files or folders by name. Globs support `*` and `?`.

```json
{
	"mapping": {
		"src": "ReplicatedStorage/Source"
	},
	"customRoutes": {
		"server": "ServerScriptService",
		"client": "StarterPlayerScripts",
		"*.server.luau": "ServerScriptService"
	}
}
```

Example input:

```text
src/features/combat/server/DamageService.luau
src/features/combat/client/CombatController.luau
```

Output:

```text
ServerScriptService/features/combat/DamageService
StarterPlayerScripts/features/combat/CombatController
```

Folder routes move the folder contents, not the folder itself. File routes move one file. The first matching route wins.

## Files on disk

Script suffixes decide instance type:

- `.server.luau`: `Script`
- `.client.luau`: `LocalScript`
- `.luau`: `ModuleScript`

Everything else is stored as `.rbxm`.

Subtrees without scripts stay packed as one `.rbxm`. Subtrees with scripts become folders with editable `.luau` files and backing `.rbxm` files. Empty folders are skipped.

## Duplicate names

Roblox allows duplicate sibling names. Filesystems do not.

Fuse keeps the first name and suffixes the rest:

```text
Block.rbxm
Block_2.rbxm
Block_3.rbxm
```

For models, the real Roblox name stays inside the `.rbxm`, so this round-trips. Scripts use the filename as the instance name, so case-only duplicates cannot fully round-trip on a case-insensitive filesystem.

## License

MPL-2.0. See [LICENSE](LICENSE).
