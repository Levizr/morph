# Migration Guide: Python Morph → morph

This guide helps you migrate from the legacy Python-based `morph` CLI to the new Rust-based `morph`.

> **Migration is in progress, not finished.** Direct file morphing (`morph app.ts --to cpp`) is fully owned by Rust, but GUI application builds still need the Python CLI for parts of the pipeline. See [What Still Needs the Python CLI](#what-still-needs-the-python-cli) before uninstalling anything.

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
| `morph translate file.ts --to cpp` | `morph file.ts --to cpp` | New flags: `--to`, `--optimize` |
| `morph pkg add` | *(not yet)* | Package manager in development |

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
morph app.ts --optimize   # intent-based codegen
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

# Rust: 100ms debounce, notify crate, Unix/TCP socket
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
- IPC: Unix socket `.morph/dev.sock` (Linux/macOS), TCP `127.0.0.1:3000` (Windows)

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
- [ ] Remove Python from build environment (direct file morphing only — keep it for `.mx` project builds, see above)
- [ ] Test dev mode: `morph dev`
- [ ] Test build: `morph build --static`
- [ ] Verify binary runs on clean machine

## What Still Needs the Python CLI

The `morph/` Python package is still present in the repo on purpose. The Rust CLI owns direct file morphing end to end, but the GUI application pipeline is split — removing Python today would silently break project builds (mispositioned widgets, untranslated logic, no hot reload) with no failing test to catch it.

| Area | Rust status | Python still needed | Why |
|---|---|---|---|
| Direct file morph (`morph app.ts`) | ✅ Complete (`morpher`, `--type`/`--optimize`) | No | Covered by 21 fixtures + 4 regression tests, outputs match Node.js |
| GUI layout engine | ❌ Missing | **Yes** — `morph/layout/engine.py` | Measure + layout pass writes real `x/y/w/h`; Rust `IRBuilder` hardcodes `0.0`, so every widget would pile at the origin |
| JS logic inside GUI builds | ❌ Not wired | **Yes** — `morph/js/codegen.py` | `morph build` never calls `morpher`; `morph-codegen` has only a string-level `translate_js` shim and emits `premain` bodies as raw JS verbatim (won't compile) |
| Dev hot reload | ❌ Stub | **Yes** — `morph/dev/` | Rust `dev` verifies IR and stops: no logic-TU emit, no `.so` compile, no IPC push (`dev.rs:118-119`); Python compiles `logic.<hash>.so` and `dlopen`s it live |
| Props / events / effects | ⚠️ Partial | **Yes** | Rust drops `onChange`/`onFocus`/keys, `effect_decls`, `global_vars`, `function_declarations`; Python `jsx_walker` + `IRBuilder` handle the full surface |
| PyPI distribution | ❌ Rust-only | **Yes** — `python-publish.yml` | `levizr-morph` releases still ship from Python; no Rust release workflow exists yet |
| Unit / integration tests | ❌ Unported | **Yes** — `tests/unit`, `tests/integration` | 14 suites import `morph.*` directly; only the translate suite runs on the Rust binary |

The rule of thumb: **file morphing → Rust; `morph build` / `morph run` / `morph dev` on `.mx` projects → keep Python installed** until the parity harness (translating GUI snippets through both translators and diffing) and the layout-engine port land. That sequence is tracked in [What Morph Is Building Right Now](../roadmap/under-construction.md).

## Rollback

If you need the Python toolchain:

```bash
pip install levizr-morph==0.0.6  # last Python release
```

The Python version is published as `morph-legacy` on PyPI for reference.

## Getting Help

- `morph --help` — all commands
- `morph doctor` — verify system
- `morph check` — lint your code
- GitHub Issues: https://github.com/Levizr/morph/issues
- Email: suggestions.morph@levizr.com