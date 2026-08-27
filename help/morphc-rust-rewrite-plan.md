# Morph Rust Rewrite Plan

## Naming: `morphc` (Morph Compiler)

- Binary: `morphc`, Crate: `morph-compiler`
- Follows `rustc`, `gcc` convention
- ~10-15MB lightweight binary, no embedded runtime

---

## Architecture Overview

### Pipeline
```
.mx files → Oxc (JSX/TS) + lightningcss (CSS) → Typed AST → IR → Tera templates → C++/Rust → g++/clang++/rustc
```

### Key Design Principles
1. **Lightweight binary** (~10-15MB) - no embedded runtime
2. **Runtime downloaded per-project** - versioned, cached globally in `~/.morph/`
3. **Global cache** - never re-download same version
4. **Semantic versioning** for everything
5. **Single repo** (`levizr/morph`) - all components
6. **Version files** control releases - edit JSON, push, GitHub Actions builds

---

## Repository Structure

```
morph/                              # Single repo, everything here
├── Cargo.toml                      # Rust workspace
├── crates/
│   ├── morphc/                     # CLI binary (~10-15MB)
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── commands/
│   │   │   │   ├── init.rs
│   │   │   │   ├── install.rs
│   │   │   │   ├── dev.rs
│   │   │   │   ├── build.rs
│   │   │   │   ├── run.rs
│   │   │   │   ├── update.rs
│   │   │   │   ├── check.rs
│   │   │   │   ├── doctor.rs
│   │   │   │   └── translate.rs
│   │   │   ├── cache.rs            # Global cache manager (~/.morph/)
│   │   │   └── versions.rs         # Version compatibility checking
│   │   └── Cargo.toml
│   ├── morph-parser/               # Oxc + lightningcss
│   │   ├── src/lib.rs
│   │   ├── src/jsx.rs
│   │   └── src/css.rs
│   ├── morph-ir/                   # Intermediate Representation
│   │   ├── src/lib.rs
│   │   ├── src/node.rs
│   │   ├── src/style.rs
│   │   ├── src/event.rs
│   │   ├── src/animation.rs
│   │   └── src/builder.rs
│   ├── morph-codegen/              # C++ / Rust code generation
│   │   ├── src/lib.rs
│   │   ├── src/emitter.rs
│   │   ├── src/node_emitter.rs
│   │   ├── src/logic_emitter.rs
│   │   ├── src/feature_set.rs
│   │   └── templates/
│   │       ├── app_main.cpp.tera
│   │       ├── window.cpp.tera
│   │       ├── node_rect.cpp.tera
│   │       ├── node_text.cpp.tera
│   │       └── node_viewport.cpp.tera
│   ├── morph-build/                # Build system
│   │   ├── src/lib.rs
│   │   ├── src/compiler.rs
│   │   ├── src/platform.rs
│   │   └── src/static_deps.rs
│   └── morph-config/               # Config + lock file parsing
│       ├── src/lib.rs
│       └── src/schema.rs
├── runtime/
│   ├── cpp/                        # C++ runtime source
│   │   ├── include/
│   │   ├── src/
│   │   ├── vendor/
│   │   └── CMakeLists.txt
│   └── rust/                       # Future Rust runtime
│       └── ...
├── versions/                       # VERSION FILES - controls releases
│   ├── morphc/
│   │   └── version.json
│   └── runtime/
│       ├── cpp.json
│       └── rust.json               # future
├── templates/
│   └── default/
├── .github/workflows/
│   ├── release-morphc.yml          # Triggered when versions/morphc/version.json changes
│   ├── release-runtime.yml         # Triggered when versions/runtime/*.json changes
│   └── ci.yml                      # PR checks
└── morphc-build-in-rust.md
```

---

## Version Files (Release Trigger System)

### Structure
```
versions/
├── morphc/
│   └── version.json
└── runtime/
    ├── cpp.json
    └── rust.json                   # future
```

### Version File Format
```json
{
  "version": "0.3.0",
  "changelog": "Added signal() support, improved render performance",
  "breaking": false
}
```

### How Releases Work
1. You make changes to code
2. When ready to release, edit version file:
   ```bash
   echo '{"version":"0.3.0","changelog":"Added signal()","breaking":false}' > versions/runtime/cpp.json
   git commit -am "release: runtime cpp 0.3.0"
   git push
   ```
3. GitHub Actions detects version change → builds → publishes release
4. No manual trigger needed - version file IS the trigger

### GitHub Actions Detection Logic
```yaml
- name: Check version change
  id: check
  run: |
    CURRENT=$(cat versions/runtime/cpp.json | jq -r .version)
    LAST=$(gh release view runtime-cpp-v$CURRENT --json tagName 2>/dev/null || echo "none")
    
    if [ "$LAST" == "none" ]; then
      echo "changed=true" >> $GITHUB_OUTPUT
      echo "version=$CURRENT" >> $GITHUB_OUTPUT
    fi
```

---

## Global Cache Structure (`~/.morph/`)

```
~/.morph/
├── config.json                     # User preferences (optional)
├── cache/
│   ├── runtimes/
│   │   ├── cpp/
│   │   │   ├── v0.1.0/
│   │   │   │   ├── include/
│   │   │   │   ├── src/
│   │   │   │   ├── vendor/
│   │   │   │   └── manifest.json   # {"version":"0.1.0","sha256":"...","size":12345}
│   │   │   ├── v0.2.0/
│   │   │   └── v0.3.0/
│   │   └── rust/                   # future
│   └── index.json                  # Available versions (fetched periodically)
├── downloads/                       # Temporary download storage
└── logs/
```

### Cache Logic
- Check global cache first (`~/.morph/cache/runtimes/{type}/v{version}/`)
- If exists → symlink/copy to `.morph/runtime/`
- If not → download from GitHub Releases → extract to global cache → link to project
- Never re-download same version + sha256 combination

---

## Project Structure

```
my-app/
├── src/
│   ├── App.mx
│   └── components/
├── morph.config.json               # Project config (runtime version pinned)
├── morph.lock                      # Locked versions + hashes
├── .morph/
│   ├── runtime/                    # Runtime sources (from global cache)
│   │   ├── include/
│   │   ├── src/
│   │   └── version.json
│   ├── build/
│   │   ├── devrt/
│   │   ├── src/                    # Generated C++
│   │   └── app                     # Production binary
│   ├── cache/
│   │   └── css/
│   └── logic/
│       └── *.cpp
└── .gitignore
```

---

## Config Files

### morph.config.json
```json
{
  "entry": "src/App.mx",
  "runtime": {
    "type": "cpp",
    "version": "0.2.0"
  },
  "window": {
    "width": 800,
    "height": 600,
    "title": "My App"
  },
  "build": {
    "output": ".morph/build/app",
    "static": false,
    "upx": false
  }
}
```

### morph.lock
```json
{
  "runtime": {
    "type": "cpp",
    "version": "0.2.0",
    "sha256": "abc123def456...",
    "downloaded_at": "2025-01-15T10:30:00Z"
  },
  "generated_by": "morphc 0.3.0"
}
```

### versions/runtime/cpp.json (in repo)
```json
{
  "version": "0.3.0",
  "changelog": "Added signal() support, improved render performance",
  "breaking": false
}
```

### versions/morphc/version.json (in repo)
```json
{
  "version": "0.4.0",
  "changelog": "New update system, faster builds",
  "breaking": false
}
```

---

## CLI Commands

### `morph init [name]`
```bash
$ morph init my-app

  Creating project "my-app"...
  ✓ Created src/App.mx
  ✓ Created morph.config.json
  ✓ Created .gitignore
  ✓ Created .morph/ directory

  Next steps:
    $ cd my-app
    $ morph install          # Download runtime sources
```

**Interactive prompt:**
```
  Install runtime sources now? [Y/n]: Y

  Downloading runtime cpp v0.2.0...
  ✓ Downloaded (2.3 MB)
  ✓ Cached to ~/.morph/cache/runtimes/cpp/v0.2.0/
  ✓ Linked to .morph/runtime/

  Done! Run 'morph dev' to start developing.
```

If user chooses N:
```
  Ok, run 'morph install' later when ready.
  $ cd my-app
  $ morph install
```

### `morph install`
```bash
$ morph install

  Reading morph.config.json...
  Runtime: cpp v0.2.0

  Checking global cache...
  ✓ Found in cache: ~/.morph/cache/runtimes/cpp/v0.2.0/
  ✓ Linked to .morph/runtime/

  Done! Run 'morph dev' to start.
```

**First time (not cached):**
```bash
$ morph install

  Runtime: cpp v0.2.0
  Checking cache... not found

  Downloading from GitHub Releases...
  ✓ Downloaded morph-runtime-cpp-v0.2.0.tar.gz (2.3 MB)
  ✓ Extracted to ~/.morph/cache/runtimes/cpp/v0.2.0/
  ✓ Linked to .morph/runtime/

  Done! Run 'morph dev' to start.
```

### `morph update`
```bash
$ morph update

  Checking for updates...

  morphc:        v0.3.0 (latest: v0.3.0) ✓
  Runtime:       v0.2.0 (latest: v0.3.0)

  New runtime available: v0.3.0
  Run 'morph update --runtime' to update.
```

### `morph update --runtime`
```bash
$ morph update --runtime

  Current runtime: v0.2.0
  Latest runtime:  v0.3.0

  Downloading runtime v0.3.0...
  ✓ Downloaded (2.4 MB)
  ✓ Cached to ~/.morph/cache/runtimes/cpp/v0.3.0/

  Updating morph.config.json...
  ✓ Updated runtime.version from "0.2.0" to "0.3.0"

  Migration Notes (v0.2.0 → v0.3.0):
  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  ✓ No breaking changes

  Deprecated (will be removed in v0.5.0):
    - `morphState()` → use `morph.state()` instead

  Run 'morph check --migrate' to auto-fix your code.

  Restart 'morph dev' to use new runtime.
```

### `morph update --self`
```bash
$ morph update --self

  Current morphc: v0.2.0
  Latest morphc:  v0.3.0

  Downloading morphc v0.3.0...
  ✓ Downloaded morphc-linux-x64 (12.3 MB)
  ✓ Installed to /usr/local/bin/morphc

  Updated! Run 'morphc --version' to verify.
```

### `morph dev`
```bash
$ morph dev

  Morph v0.3.0 | Runtime cpp v0.2.0

  Checking runtime... ✓
  Parsing .mx files... ✓ (12ms)
  Building IR... ✓ (8ms)

  Starting dev server on port 127.0.0.1:3000...
  Watching src/ for changes...
```

**If runtime outdated:**
```
  ⚠ Runtime v0.1.0 is deprecated for morphc v0.3.0
    Run `morph update --runtime` to get v0.2.0
```

### `morph build`
```bash
$ morph build

  Morph v0.3.0 | Runtime cpp v0.2.0

  ✓ Parsed 3 files
  ✓ IR built
  ✓ C++ generated (45 files)
  ✓ Compiled with g++ (2.1s)
  ✓ Binary: .morph/build/app (1.8 MB)
```

### `morph run`
```bash
$ morph run
# Runs morph build, then executes the binary
```

### `morph check`
```bash
$ morph check

  Checking src/App.mx... ✓
  Checking src/components/Button.mx...
  ⚠ Line 15: `morphState()` is deprecated
    Use `morph.state()` instead

  Found 1 warning
```

### `morph check --migrate`
```bash
$ morph check --migrate

  Migrating src/components/Button.mx...
  ✓ Replaced morphState() with morph.state()

  Migration complete. 1 file updated.
```

### `morph doctor`
```bash
$ morph doctor

  ✓ g++ 13.2.0
  ✓ cmake 3.28.1
  ✓ pkg-config 0.29.2
  ✓ GLFW 3.3.8
  ✓ FreeType 2.13.1
  ✓ HarfBuzz 8.3.0
  ✓ OpenGL (NVIDIA 535.129.03)

  All checks passed!
```

### `morph translate <file>`
```bash
$ morph translate src/logic.ts

  Parsing... ✓ (3ms)
  Translating to C++...
  ✓ Generated: .morph/build/src/logic.cpp
```

---

## Version Compatibility System

### Compatibility Rules
```json
{
  "version": "0.2.0",
  "morphc_min": "0.2.0",
  "morphc_max": "0.4.0"
}
```

### Warning Levels
```
✓ Compatible:     Runtime works with this morphc version
⚠ Deprecated:     Works but will be removed in future
✗ Incompatible:   Cannot use, must update
```

### Examples
```
morphc v0.3.0 + runtime v0.2.0 → ✓ (min=0.2.0, max=0.4.0)
morphc v0.3.0 + runtime v0.1.0 → ⚠ (max=0.2.0, deprecated)
morphc v0.3.0 + runtime v0.4.0 → ✗ (min=0.4.0, incompatible)
```

---

## GitHub Actions Workflows

### 1. `release-morphc.yml` (Auto-triggered on version file change)
```yaml
name: Release morphc
on:
  push:
    paths: ['versions/morphc/version.json']

jobs:
  build:
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
            artifact: morphc-linux-x64
          - target: aarch64-unknown-linux-gnu
            os: ubuntu-latest
            artifact: morphc-linux-arm64
          - target: x86_64-apple-darwin
            os: macos-latest
            artifact: morphc-macos-x64
          - target: aarch64-apple-darwin
            os: macos-latest
            artifact: morphc-macos-arm64
          - target: x86_64-pc-windows-msvc
            os: windows-latest
            artifact: morphc-windows-x64.exe
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release -p morphc --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ matrix.artifact }}
          path: target/${{ matrix.target }}/release/morphc*

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Get version info
        id: version
        run: |
          VERSION=$(jq -r .version versions/morphc/version.json)
          CHANGELOG=$(jq -r .changelog versions/morphc/version.json)
          BREAKING=$(jq -r .breaking versions/morphc/version.json)
          echo "version=$VERSION" >> $GITHUB_OUTPUT
          echo "changelog=$CHANGELOG" >> $GITHUB_OUTPUT
          echo "breaking=$BREAKING" >> $GITHUB_OUTPUT
      - uses: actions/download-artifact@v4
      - uses: softprops/action-gh-release@v1
        with:
          tag_name: v${{ steps.version.outputs.version }}
          name: morphc v${{ steps.version.outputs.version }}
          body: |
            ${{ steps.version.outputs.changelog }}
            
            Breaking: ${{ steps.version.outputs.breaking }}
          files: morphc-*/morphc*
```

### 2. `release-runtime.yml` (Auto-triggered on runtime version change)
```yaml
name: Release Runtime
on:
  push:
    paths: ['versions/runtime/*.json']

jobs:
  bundle:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Detect changed runtime
        id: detect
        run: |
          CHANGED=$(git diff --name-only HEAD~1 | grep "versions/runtime/" | head -1)
          RUNTIME=$(basename $CHANGED .json)
          VERSION=$(jq -r .version $CHANGED)
          CHANGELOG=$(jq -r .changelog $CHANGED)
          echo "runtime=$RUNTIME" >> $GITHUB_OUTPUT
          echo "version=$VERSION" >> $GITHUB_OUTPUT
          echo "changelog=$CHANGELOG" >> $GITHUB_OUTPUT
      - name: Bundle runtime
        run: |
          tar -czf morph-runtime-${{ steps.detect.outputs.runtime }}-v${{ steps.detect.outputs.version }}.tar.gz \
            -C runtime/${{ steps.detect.outputs.runtime }} .
      - name: Upload to GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          tag_name: runtime-${{ steps.detect.outputs.runtime }}-v${{ steps.detect.outputs.version }}
          name: Runtime ${{ steps.detect.outputs.runtime }} v${{ steps.detect.outputs.version }}
          body: ${{ steps.detect.outputs.changelog }}
          files: morph-runtime-*.tar.gz
```

### 3. `ci.yml` (PR checks)
```yaml
name: CI
on:
  pull_request:
    branches: [main]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo check --workspace
      - run: cargo test --workspace
      - run: cargo clippy --workspace -- -D warnings
```

### Build Trigger Strategy
```
Edit versions/morphc/version.json + push → Build morphc binaries
Edit versions/runtime/cpp.json + push   → Bundle cpp runtime
Edit versions/runtime/rust.json + push  → Bundle rust runtime
PR to main                              → CI checks only (no release)
```

**No manual triggers needed** - version files ARE the triggers.

---

## Binary Distribution

### Install Script (get.morph.dev)
```bash
#!/bin/bash
set -e

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$ARCH" in
  x86_64)  ARCH="x64" ;;
  aarch64) ARCH="arm64" ;;
  *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
  linux)  PLATFORM="linux-${ARCH}" ;;
  darwin) PLATFORM="macos-${ARCH}" ;;
  *)      echo "Unsupported OS: $OS"; exit 1 ;;
esAC

echo "Installing morphc for ${PLATFORM}..."

curl -fsSL "https://github.com/levizr/morph/releases/latest/download/morphc-${PLATFORM}.tar.gz" | tar xz
sudo mv morphc /usr/local/bin/
chmod +x /usr/local/bin/morphc

echo "morphc installed! Run 'morph init' to get started."
```

### Binary Sizes
| Component | Size |
|-----------|------|
| morphc binary | ~10-15MB |
| Runtime (downloaded) | ~2-3MB compressed, ~8-10MB extracted |
| Per project total | ~12-25MB |

---

## Python Version Handling

1. **Delete `morph/` directory** from repo
2. **Publish as `morph-legacy`** on PyPI later for reference
3. **README note**: "Python version deprecated. Use morphc (Rust)."

---

## Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [x] Set up Cargo workspace
- [x] Implement `morphc` CLI with clap
- [x] Implement `morph-config` (config + lock file parsing)
- [x] Implement global cache system (`~/.morph/`)
- [x] Implement `morph init` with interactive prompt
- [x] Implement `morph install` (download + extract + cache + link)

### Phase 2: Parser (Week 3-4)
- [x] Implement `morph-parser` with Oxc (JSX/TSX)
- [x] Implement `morph-parser` with lightningcss (CSS)
- [x] JSX walking and component extraction
- [x] CSS property parsing

### Phase 3: AST + IR (Week 5-6)
- [x] Port Python AST nodes to Rust (`morph-ir`)
- [x] Port IR nodes (IRNode, IRWindow, IRPage)
- [x] Port style system (CSS property registry)
- [x] Port Tailwind resolver

### Phase 4: Codegen (Week 7-8)
- [x] Port Jinja2 templates to Tera
- [x] Implement C++ code generation
- [x] Port feature flag detection
- [x] Port event/reactive logic emission

### Phase 5: Build System (Week 9-10)
- [x] Implement `morph-build` (compiler invocation)
- [x] Port platform detection
- [x] Implement `morph build` and `morph run` (output `.morph/output/<clean>`)
- [ ] Implement `morph dev` with hot reload — stub with notify+IPC

### Phase 5: Build System (Week 9-10)
- [ ] Implement `morph-build` (compiler invocation)
- [ ] Port platform detection
- [ ] Implement `morph build` and `morph run`
- [ ] Implement `morph dev` with hot reload

### Phase 6: Polish (Week 11-12)
- [ ] Implement `morph update --runtime` and `--self`
- [ ] Implement `morph check` with auto-migration
- [ ] Implement `morph doctor`
- [ ] GitHub Actions workflows
- [ ] Install script (get.morph.dev)
- [ ] Documentation

---

## Summary: Developer Workflow

### For morphc developer (you):
```bash
# Make changes
git commit -am "feat: added signal() support"

# Release runtime
echo '{"version":"0.3.0","changelog":"Added signal()","breaking":false}' > versions/runtime/cpp.json
git commit -am "release: runtime cpp 0.3.0"
git push
# → GitHub Actions builds runtime v0.3.0

# Release morphc (when ready)
echo '{"version":"0.4.0","changelog":"New features","breaking":false}' > versions/morphc/version.json
git commit -am "release: morphc 0.4.0"
git push
# → GitHub Actions builds morphc v0.4.0 for all platforms
```

### For end user:
```bash
curl -fsSL https://get.morph.dev | sh           # Install morphc
morph init my-app                                # Create project
cd my-app
Y (install now)                                  # Download runtime
morph dev                                        # Start developing

# Later, update runtime
morph update --runtime

# Update morphc itself
morph update --self
```
