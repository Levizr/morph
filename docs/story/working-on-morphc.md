# Working on morphc — The Rust Rewrite

**Part of:** [The Story of Morph](index.md)

> This page documents what I'm actively building right now — the Rust rewrite of Morph's Python toolchain. This is living documentation; I update it as decisions are made and work progresses.

## What's happening

The Python toolchain that compiles `.mx` files into native binaries is being rewritten in Rust. The new binary is called **`morphc`** (Morph Compiler) — a single, lightweight binary that replaces the entire Python dependency.

**Why?** Python was the right choice to prove the concept. Now the concept is proven, and compile speed matters. A Rust compiler removes the Python dependency, makes `morph dev` instant, and gives users a single binary to install.

## Architecture

### Pipeline (before → after)

```
Before (Python):  .mx → tree-sitter → Python AST → IR → Jinja2 → C++ → g++
After (Rust):     .mx → Oxc → Typed AST → IR → Tera → C++ → g++/clang++
```

### Key decisions

| Decision | Choice | Why |
|----------|--------|-----|
| JSX/TSX parser | **Oxc** | 3x faster than SWC, arena-allocated, spec-compliant |
| CSS parser | **lightningcss** | Extremely fast, typed property values, browser-grade |
| Template engine | **Tera** | Jinja2-compatible syntax, fast runtime compilation |
| CLI framework | **clap** (derive) | Industry standard, derive macros, completions |
| Runtime naming | `cpp` / `rust` | Simple, clear |
| Binary name | `morphc` | Follows `rustc`, `gcc` convention |

### Repository structure (single repo)

Everything lives in `levizr/morph` — no separate repos:

```
morph/
├── Cargo.toml              # Rust workspace
├── crates/
│   ├── morphc/             # CLI binary (~10-15MB)
│   ├── morph-parser/       # Oxc + lightningcss
│   ├── morph-ir/           # Intermediate Representation
│   ├── morph-codegen/      # C++ / Rust code generation
│   ├── morph-build/        # Build system
│   └── morph-config/       # Config + lock file parsing
├── runtime/
│   ├── cpp/                # C++ runtime source
│   └── rust/               # Future Rust runtime
├── versions/               # Version files (release triggers)
│   ├── morphc/
│   │   └── version.json
│   └── runtime/
│       ├── cpp.json
│       └── rust.json       # future
└── morph/                  # Current Python (deprecated)
```

### .morph directory (per-project)

```
my-app/
├── src/App.mx
├── morph.config.json
├── morph.lock
└── .morph/
    ├── runtime/            # Downloaded runtime (from global cache)
    ├── build/              # Build artifacts
    └── cache/              # Project-specific cache
```

### Global cache (`~/.morph/`)

```
~/.morph/
├── cache/
│   └── runtimes/
│       ├── cpp/
│       │   ├── v0.1.0/
│       │   ├── v0.2.0/
│       │   └── v0.3.0/
│       └── rust/           # future
└── index.json
```

## CLI commands

| Command | What it does |
|---------|--------------|
| `morph init [name]` | Create project, scaffold files, prompt to install runtime |
| `morph install` | Download runtime from GitHub Releases, cache globally |
| `morph update --runtime` | Update runtime version, show migration notes |
| `morph update --self` | Update morphc binary |
| `morph dev` | Start dev mode with hot reload |
| `morph build` | Compile .mx → native binary |
| `morph run` | Build + run |
| `morph check` | Lint .mx files |
| `morph check --migrate` | Auto-fix deprecated patterns |
| `morph doctor` | Check system dependencies |
| `morph translate <file>` | Translate .ts/.js → C++ |

## Version system

### Two separate versions

- **morphc binary** — the CLI tool (e.g., `v0.3.0`)
- **Runtime** — the C++/Rust runtime source (e.g., `v0.2.0`)

### Version files (release triggers)

```json
// versions/runtime/cpp.json
{
  "version": "0.3.0",
  "changelog": "Added signal() support, improved render performance",
  "breaking": false
}
```

Edit the file, push, GitHub Actions builds automatically. No manual trigger needed.

### Compatibility

```
morphc v0.3.0 + runtime v0.2.0 → ✓ Works
morphc v0.3.0 + runtime v0.1.0 → ⚠ Deprecated (update recommended)
morphc v0.3.0 + runtime v0.4.0 → ✗ Incompatible (update morphc)
```

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
  }
}
```

## Version file security

The version files live in a public repo. Here's how we prevent bad releases:

### 1. Branch protection
- `main` branch requires PR review before merge
- Status checks must pass before merge
- Only repo owner can merge

### 2. CODEOWNERS
```
# .github/CODEOWNERS
versions/**    @Piyushthelagend
```
Any PR touching `versions/` requires explicit approval from the maintainer.

### 3. Version format validation in CI
```yaml
- name: Validate version format
  run: |
    VERSION=$(jq -r .version versions/runtime/cpp.json)
    if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
      echo "Invalid semver: $VERSION"
      exit 1
    fi
```
Invalid version = build fails, no release published.

### 4. Only `main` triggers builds
```yaml
on:
  push:
    branches: [main]
    paths: ['versions/**']
```
PRs never trigger releases. Only merged code on `main`.

### 5. What if a bad PR merges?
```
Bad PR merges → versions/runtime/cpp.json has "version": "999.99.99"
              → GitHub Actions validates → FAILS (invalid or mismatched tag)
              → No release published
              → Fix it, push correct version
```

### 6. Optional: signed releases
Sign artifacts with GPG key for production releases. Users verify before installing.

### Worst case
Invalid version gets a GitHub Release with broken artifacts. But users are protected because:
- `morph install` checks sha256 hash
- morphc verifies runtime compatibility
- Bad releases can be deleted manually

## What stays the same for users

- `.mx` files, CSS, Tailwind — unchanged
- `morphState`, `morphEffect` — unchanged
- `morph dev`, `morph build` — same commands, same flags
- JSON IR wire format — unchanged
- Dev-mode socket protocol — unchanged

## What changes

- **No Python dependency** — single binary install
- **Much faster** — Oxc parsing, parallel compilation, no interpreter startup
- **Global cache** — never re-download the same runtime version
- **Better DX** — `morph init` prompts to install, `morph update` handles versions
- **Version management** — deprecation warnings, migration notes, compatibility checks

## Justice for morph-legacy

The Python toolchain served Morph well. It proved the concept, enabled rapid iteration, and made the project possible. When the Rust rewrite ships, the Python code won't be deleted from history — it will be published as **`morph-legacy`** on PyPI for anyone who wants to study it, reference it, or understand how Morph started.

The Python version was never meant to ship to end users. It was the prototype that became the blueprint. The Rust version is the production tool that honors that work by being everything Python couldn't be: fast, self-contained, and zero-dependency.

> `morph-legacy` will be published as-is, with a README pointing to `morphc`. No maintenance, no updates — just preserved history.

## Implementation phases

- [x] Phase 1: Foundation (CLI, config, parser, init/install)
- [x] Phase 2: Parser (Oxc + lightningcss)
- [x] Phase 3: AST + IR (style registry, Tailwind, IRBuilder — verified vs Python)
- [ ] Phase 4: Codegen (Tera templates) — in progress
- [ ] Phase 5: Build system
- [ ] Phase 6: Dev mode
- [ ] Phase 7: Polish (check, doctor, CI/CD, distribution)

## Full implementation plan

For the complete technical plan — crate structure, all CLI commands, config files, GitHub Actions workflows, binary distribution, and implementation phases — see the [full rewrite plan](../../help/morphc-rust-rewrite-plan.md) in the help section.

## Follow the progress

This page will be updated as work progresses. The best way to follow along is watching the repo and checking back here.

---

> Built with frustration, curiosity, and a refusal to accept "impossible" — [PIYUSH](https://github.com/Piyushthelagend)
