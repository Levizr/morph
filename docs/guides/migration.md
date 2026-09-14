# Migration Guide: Python Morph → morph

This guide helps you migrate from the legacy Python-based `morph` CLI to the new Rust-based `morph`.

> **Migration complete.** The Rust `morph` binary owns the entire pipeline — direct file morphing and `.mx` project builds. Python was removed from the repository in September 2026. There is nothing to keep installed.

## Quick Command Mapping

| Python `morph` | Rust `morph` | Notes |
|---|---|---|
| `morph init my-app` | `morph new my-app` | Renamed `init` → `new` |
| `morph dev` | `morph dev` | Same |
| `morph build` | `morph build` | Same |
| `morph run` | `morph run` | Same |
| `morph check` | `morph check` | Same |
| `morph doctor` | `morph doctor` | Same |
| `morph cache` | `morph cache` | Same |
| `morph translate file.ts` | `morph file.ts` | Direct file morphing, no subcommand |
| `morph translate file.ts --to cpp` | `morph file.ts --to cpp` | New flag: `--to` |

## Installation Changes

### Before (Python)

```bash
pip install levizr-morph
# or
pip install "morph @ git+https://github.com/Levizr/morph.git"
```

### After (Rust)

```bash
# Stable (installs the `morph` binary from the `morphc` package)
cargo install morphc

# From source (latest)
git clone https://github.com/Levizr/morph.git
cd morph
cargo install --path crates/morphc
```

**No Python required.** No `pip`, no virtual environments, no `python3` on PATH.

## Configuration Changes

### `morph.config.json`

```json
{
  "name": "my-app",
  "entry": "src/App.mx",
  "output": "dist/",
  "window": { "width": 800, "height": 600, "title": "My App" },
  "renderer": "flash",
  "runtime": { "type": "cpp", "version": "0.1.0" },
  "build": {
    "upx": true,
    "cxx": "",
    "dev_cxx": ""
  },
  "lint": { "disable": [], "severities": {} }
}
```

**New fields:**
- `runtime.type` / `runtime.version` — replaces implicit runtime download
- `build.cxx` / `build.dev_cxx` — separate production vs dev compilers
- `output` default changed from `dist/` to `.morph/output/`

**Removed fields:**
- `dependencies` — package manager not yet implemented
- `node_bridge` — removed (was Python-only)

### New File: `morph.lock`

Generated on first `morph dev`/`build`/`install`. Commits exact runtime version + SHA256 for reproducibility.

```json
{
  "runtime": { "type": "cpp", "version": "0.1.0", "sha256": "...", "downloaded_at": "..." },
  "generated_by": "morph 0.3.0",
  "generated_at": "..."
}
```

**Commit this file.**

## CLI Behavior Changes

### Direct File Morphing

```bash
# Python: required subcommand
morph translate app.ts --to cpp

# Rust: direct arguments
morph app.ts              # → app.cpp
morph app.ts --to cpp     # → app.cpp
morph app.ts --to rust    # → app.rs (experimental)
```

### Project Commands

```bash
# Python
morph init my-app
morph init . --width 1024 --height 768

# Rust
morph new my-app
morph new . --width 1024 --height 768 --ext mx -y
```

### Dev Mode

```bash
# Python: watched via inotify + custom Python watcher
morph dev

# Rust: 100ms debounce, notify crate, loopback TCP IPC
morph dev
morph dev --entry src/App.mx
```

### Build Flags

```bash
# Python
morph build --static --no-upx --output bin/

# Rust (same flags)
morph build --static --no-upx --output bin/
```

## Runtime System

### Before

- Runtime bundled with Python package
- Downloaded to project on `morph dev`
- No version locking

### After

- Runtime published as GitHub Releases (tagged by `versions/runtime/cpp.json`)
- Global cache at `~/.morph/cache/runtimes/cpp/vX.Y.Z/`
- Project symlinks to cache: `.morph/runtime/ → ~/.morph/cache/runtimes/cpp/v0.1.0/`
- `morph.lock` pins exact version
- `morph install` downloads explicitly
- `morph update --runtime` upgrades

## Lint Rules

Rule codes unchanged. Configuration moved to `lint` section:

```json
{
  "lint": {
    "disable": ["mx-list-key"],
    "severities": { "mx-tag-stub": "error" }
  }
}
```

Run `morph check` for lint-only pass.

## Dev Runtime (`morph_devrt`)

- Still auto-built via CMake on first `morph dev`
- Source hash tracking in `.morph/hash/dev.fingerprint`
- Rebuilds when runtime source changes
- IPC: loopback TCP `127.0.0.1:39573` (ephemeral fallback on collision)

## File Structure Changes

```
# Before (Python)
my-app/
├── src/App.mx
├── style.css
├── morph.config.json
├── dist/app

# After (Rust)
my-app/
├── src/App.mx
├── style.css
├── env.d.ts
├── morph.config.json
├── morph.lock          # NEW
├── tsconfig.json
├── .morph/
│   ├── runtime/        # symlink to global cache
│   ├── build/          # artifacts, logic.so
│   └── cache/
│       ├── css/        # fetched CSS
│       └── *.fingerprint
└── .morph/output/app   # or dist/ if configured
```

## CI/CD Updates

### GitHub Actions

```yaml
# Before
- name: Install Morph
  run: pip install levizr-morph

# After
- name: Install Rust
  uses: dtolnay/rust-toolchain@stable
- name: Install morph
  run: cargo install morphc
```

### Build Step

```yaml
# Before
- run: morph build --static

# After (same)
- run: morph build --static
```

## Breaking Changes Checklist

- [ ] Update `morph.config.json` with `runtime` section
- [ ] Commit `morph.lock`
- [ ] Update CI to install the `morph` binary via `cargo install morphc`
- [ ] Replace `morph translate` with `morph file.ts --to cpp`
- [ ] Replace `morph init` with `morph new`
- [ ] Remove any Python toolchain from your build environment — it is not needed (the Rust binary is self-contained)
- [ ] Test dev mode: `morph dev`
- [ ] Test build: `morph build --static`
- [ ] Verify binary runs on clean machine

## What Still Needs the Python CLI

**Nothing.** The Rust `morph` binary owns the entire pipeline — direct file morphing *and* `.mx` project builds. Python was removed from the repository in September 2026. The only Python left is the fixture-test harness under `tests/translate/`, which drives the built `morph` binary.

| Area | Rust status |
|---|---|
| Direct file morph (`morph app.ts`) | ✅ Complete (`morpher`), outputs match Node.js |
| Parse → IR → codegen → build for `.mx` projects | ✅ All in-crate (`morph-parser` → `morph-ir` → `morph-codegen` → `morph-build`) |
| Dev hot reload | ✅ `morph dev` — in-process compile + loopback TCP push to `morph_devrt` |
| CLI / distribution | ✅ `morphc` on crates.io — `cargo install morphc` |
| Tests | ✅ `cargo test --workspace` + `tests/translate` fixtures drive the Rust binary |

## Rollback

There is no separate Python toolchain to roll back to. If you need an older behavior, run an older `morphc` release (`cargo install morphc --version ...`).

## Getting Help

- `morph --help` — all commands
- `morph doctor` — verify system
- `morph check` — lint your code
- GitHub Issues: https://github.com/Levizr/morph/issues
- Email: suggestions.morph@levizr.com