# morph-config

Morph project configuration parsing — `morph.config.json` and `morph.lock`.

Part of the [Morph](https://github.com/Levizr/morph) toolchain. Parses and validates the project config that drives `morph dev` / `morph build` / `morph new`, plus the version file (`versions/`) and lock file (`morph.lock`) format.

## Install

```bash
cargo add morph-config
```

## Library Usage

```rust
use morph_config::{MorphConfig, MorphLock, VersionFile};

// Load morph.config.json
let config = MorphConfig::from_path("morph.config.json")?;
let output_dir = config.output_dir();

// Load the lock file pinned by `morph install`
let lock = MorphLock::from_path(".morph/lock.json")?;
let runtime_version = lock.runtime.version;

// Version files drive releases (versions/morphc/version.json etc.)
let vf = VersionFile::from_path("versions/runtime/version.json")?;
```

## What It Covers

- `MorphConfig` — `name`, `entry`, `output`, `window`, `renderer`, `runtime`, `build`, `lint`, `native`
- `VersionFile` — `version` + `changelog` + `breaking` (release triggers)
- `MorphLock` — exact runtime version, SHA256, and generator info for reproducibility
- Entry-point validation (`validate_entry_ext`), supported source extensions, clean app names

## License

Apache-2.0