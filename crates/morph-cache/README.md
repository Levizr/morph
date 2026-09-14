# morph-cache

Global cache management for the Morph toolchain — runtimes, fingerprints, and download safety.

Part of the [Morph](https://github.com/Levizr/morph) toolchain. Owns `~/.morph/cache/`: downloads C++ runtimes from GitHub Releases, verifies them with SHA256, symlinks them into projects, and computes fingerprints for incremental builds.

## Install

```bash
cargo add morph-cache
```

## Library Usage

```rust
use morph_cache::download_runtime;

// Downloads (if not cached) and returns the versioned runtime dir
let runtime_dir = download_runtime("cpp", "0.1.0")?;

// Link a project's .morph/runtime/ to the global cache
morph_cache::link_runtime_to_project(&runtime_dir, &project_morph_dir)?;

// Incremental-build fingerprinting
let hash = morph_cache::hash_tree(&runtime_dir);
let fp = morph_cache::fingerprint_inputs(&[("config", &config_json), ("entry", &source)]);
```

## What It Covers

- **Runtime cache** — global cache resolution (`~/.morph/cache/runtimes/`), `is_runtime_cached`, download with SHA256 verification, project symlinking, lock-file writing
- **Latest version** — `fetch_latest_runtime_version()` from the GitHub releases index
- **Fingerprinting** — `hash_tree` (dir content hashing), `fingerprint_inputs`, stored fingerprints per binary, `sha256_string` / `sha256_file` / `sha256_bytes`
- **Project state** — `is_project_runtime_installed`, hash dirs under `.morph/`

## License

Apache-2.0