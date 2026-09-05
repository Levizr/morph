# CLI Commands

Global flags: `morph --version` prints the CLI version; `morph -h` / `morph --help` lists all commands.

## Direct File Morphing (No Project Required)

Translate a `.ts` or `.js` file to C++ or Rust instantly:

```bash
morph app.ts              # → app.cpp (default target: C++)
morph app.ts --to cpp     # → app.cpp
morph app.ts --to rust    # → app.rs (experimental)
morph app.ts --to cpp --optimize  # intent-based codegen with escape analysis
```

- Only `.ts` / `.js` files allowed (`.tsx`/`.jsx`/`.mx` rejected — use `morph build` for projects)
- Output: `<basename>.cpp` or `<basename>.rs` with trailing newline
- Standalone: includes minimal shim (`morph::str`, `morph::dev_log`)
- Top-level statements wrapped in `int main()`

## Project Commands

### `morph new` — Scaffold a New Project

```bash
morph new [name]
```

| Option | Description |
|---|---|
| `name` | Project name (use `.` for current directory) |
| `--width` | Window width (default: 800) |
| `--height` | Window height (default: 600) |
| `--title` | Window title (default: project name) |
| `--entry` | Entry file (default: `src/App.mx`) |
| `--ext` | Source extension: `mx` \| `tsx` (default: `mx`) |
| `-y`, `--yes` | Skip interactive wizard, use defaults |

```bash
# Interactive
morph new my-app

# Non-interactive
morph new my-app --width 1024 --height 768 --title "My App" -y

# Into current directory
morph new . --ext tsx
```

### `morph install` — Download Runtime Sources

```bash
morph install
```

Downloads the C++ runtime (matching `morph.config.json` `runtime.version`) from GitHub Releases into the global cache `~/.morph/cache/runtimes/cpp/vX.Y.Z/` and symlinks it into `.morph/runtime/`. Creates/updates `morph.lock`.

Run this once after `morph new` or when switching runtime versions manually.

### `morph update` — Update Runtime or morph

```bash
morph update              # show current versions + available updates
morph update --runtime    # update runtime to latest compatible version
morph update --self       # update morph binary (cargo install)
```

- `--runtime`: reads `versions/runtime/cpp.json` from GitHub, rewrites `morph.config.json` + `morph.lock`, re-links runtime
- `--self`: runs `cargo install --git https://github.com/Levizr/morph.git morph`

### `morph dev` — Start Dev Mode with Hot Reload

```bash
morph dev [--entry src/App.mx]
```

- Ensures runtime is installed
- Builds dev runtime (CMake + g++/clang++)
- Starts file watcher on `src/` + entry dir (100ms debounce)
- On save: re-runs parse → CSS → IR → compiles `logic.<hash>.so` → pushes IR over Unix socket to `morph_devrt`
- IPC: Unix socket `.morph/dev.sock` (Linux/macOS), TCP `127.0.0.1:3000` (Windows)

### `morph build` — Compile Production Binary

```bash
morph build [--entry src/App.mx] [--output bin/] [--static] [--upx true|false] [--no-upx]
```

| Option | Description |
|---|---|
| `--entry` | Override entry file |
| `--output` | Output directory (default: config `output`, `.morph/output`) |
| `--static` | Statically link GLFW/FreeType/HarfBuzz (needs `.a` archives) |
| `--upx` / `--no-upx` | Override UPX compression (default: config `build.upx`) |

- Parses `.mx`/`.tsx`/`.ts` → lightningcss → IR → `CppEmitter` (Tera templates) → `app.cpp`
- Fingerprinting: if `morph.config.json` + entry + runtime hash unchanged → "Up to date — nothing to compile"
- Compiles with `-std=c++20 -O2 -ffunction-sections -fdata-sections -Wl,--gc-sections`
- Links runtime sources: core, render, reactivity, net, renderers (flash/forge)
- UPX compression applied if available

### `morph run` — Build and Run

```bash
morph run [binary] [--entry src/App.mx] [--output bin/] [--static]
```

If `binary` given, runs it directly. Otherwise builds first, then launches.

### `morph check` — Lint Source Files

```bash
morph check [PATH] [--entry src/App.mx] [--migrate]
```

- `PATH`: file or directory to check (overrides config entry)
- `--migrate`: auto-fix deprecated patterns (stub)
- Walks `src/` for `.mx`/`.tsx`/`.ts`, runs `morph-parser::linter::check`
- Aggregates `LintError` (severity, code, message, suggestion, file, line, col)
- Exit code `0` = clean, `1` = errors — CI-friendly

### `morph doctor` — Verify System Dependencies

```bash
morph doctor [-v/--verbose] [-y/--yes]
```

Checks: `g++`/`clang++`, `cmake`, `pkg-config`, GLFW, OpenGL, FreeType, HarfBuzz.

### `morph cache` — Manage Fetched CSS Cache

```bash
morph cache
```

Shows size of `.morph/cache/` and prompts to clear. Global cache `~/.morph/cache/` is read-only hint.

## Version System

Two independent versions:

- **morph binary** — the CLI tool (e.g., `v0.3.0`)
- **Runtime** — the C++ runtime source (e.g., `v0.2.0`)

### Version Files (Release Triggers)

```json
// versions/runtime/cpp.json
{
  "version": "0.3.0",
  "changelog": "Added signal() support, improved render performance",
  "breaking": false
}
```

Edit the file, push to `main`, GitHub Actions builds automatically. No manual trigger needed.

### Compatibility Matrix

```
morph v0.3.0 + runtime v0.2.0 → ✓ Works
morph v0.3.0 + runtime v0.1.0 → ⚠ Deprecated (update recommended)
morph v0.3.0 + runtime v0.4.0 → ✗ Incompatible (update morph)
```

### `morph.lock`

Generated on first `install`/`dev`/`build`:

```json
{
  "runtime": { "type": "cpp", "version": "0.1.0", "sha256": "...", "downloaded_at": "..." },
  "generated_by": "morph 0.3.0",
  "generated_at": "..."
}
```

Commit this file for reproducible builds.