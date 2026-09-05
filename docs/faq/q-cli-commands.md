# What Are the New morph Commands?

The Rust rewrite (`morph`) introduces a cleaner CLI with direct file morphing and a simplified command structure.

## Direct File Morphing (No Project Required)

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

| Command | Description |
|---|---|
| `morph new [name]` | Scaffold a new `.mx` or `.tsx` project |
| `morph install` | Download runtime sources from GitHub |
| `morph update [--runtime/--self]` | Update runtime or morph binary |
| `morph dev [--entry]` | Start dev mode with hot reload |
| `morph build [--static/--no-upx/--output]` | Compile production binary |
| `morph run [binary] [--static]` | Build and run |
| `morph check [PATH] [--migrate]` | Lint source files |
| `morph doctor [-v/-y]` | Verify system dependencies |
| `morph cache` | Manage fetched CSS cache |

### New Command Details

**`morph new`** (replaces `morph init`)

```bash
morph new my-app
morph new my-app --width 1024 --height 768 --title "My App" --ext mx -y
morph new . --ext tsx    # scaffold into current directory
```

**`morph install`** — Downloads C++ runtime matching `morph.config.json` `runtime.version` into `~/.morph/cache/runtimes/cpp/vX.Y.Z/` and symlinks to `.morph/runtime/`. Creates/updates `morph.lock`.

**`morph update`**

```bash
morph update              # show current + available versions
morph update --runtime    # upgrade runtime
morph update --self       # upgrade morph (cargo install)
```

**`morph dev`** — 100ms debounce, Unix socket (`.morph/dev.sock`), `morph_devrt` hot reload.

**`morph build`** — Fingerprinting incremental builds, UPX/static flags, feature detection.

**`morph check`** — Exit code 0 = clean, 1 = errors (CI-friendly).

## Version System

Two independent versions:
- **morph binary** — the CLI tool (e.g., `v0.3.0`)
- **Runtime** — the C++ runtime source (e.g., `v0.2.0`)

```bash
morph --version
# morph 0.3.0 (CLI)
# Runtime: cpp v0.2.0 (from morph.lock)
```

## Migration from Python `morph`

| Python `morph` | Rust `morph` |
|---|---|
| `morph init` | `morph new` |
| `morph translate file.ts` | `morph file.ts` |
| `pip install levizr-morph` | `cargo install morph` |
| `morph config.json` | `morph.config.json` + `morph.lock` |

See the [Migration Guide](../guides/migration.md) for full details.