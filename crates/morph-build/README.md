# morph-build

Morph build system — compilation, dev mode, IPC, platform, and packaging.

Part of the [Morph](https://github.com/Levizr/morph) toolchain. Drives `morph build` / `morph run` / `morph dev`: generates C++ from codegen, compiles it with g++/clang++, fingerprints for incremental builds, boots the dev renderer (`morph_devrt`) over loopback IPC, fetches remote CSS, and post-processes with UPX / static linking.

## Install

```bash
cargo add morph-build
```

## Library Usage

```rust
use morph_build::{build_project, detect_compiler, find_runtime_dir};

let cxx = detect_compiler();
let runtime_dir = find_runtime_dir(&project_dir);
check_system()?;          // g++, cmake, pkg-config, GLFW, ...
build_project(&project_dir)?;
```

## What It Covers

- **Compilation** — `Compiler`, `NativeFlags`, `BuildOptions`, `build_project`, `detect_compiler`
- **System checks** — `check_system` returns which required tools / optional libs are present
- **Dev mode** — `dev_shared_flags`, `morph_devrt` build + launch, source-hash fingerprints
- **IPC** — loopback TCP client (`IpcClient`), default port 39573, socket announce parsing
- **Logic hot reload** — `compile_logic` → shared library (`.so`/`.dylib`/`.dll`)
- **Packaging** — UPX compression (`ensure_upx`, `compress`), static dependency bundling (`StaticDeps`)
- **CSS fetching** — `CssFetcher`, remote URL rewriting, MD5 keying

## License

Apache-2.0