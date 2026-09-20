# The Build Machine: Compilers, Fingerprints, and Tiny Binaries

**Part of:** [Dev Docs](overview.md)

`morph build` looks like one command. Underneath it is a small factory: a compiler driver, a fingerprint clerk who hates redundant work, a platform diplomat, a binary dietician (UPX), and a static-linking strongman. All of it lives in `morph-build`, and all of it runs in-process inside the `morph` binary — there is no Python anywhere in this pipeline, only Rust telling `g++` what to do.

## The factory floor (`crates/morph-build/src/`)

| File | Owns |
|---|---|
| `lib.rs` | Public surface: `Compiler`, `BuildOptions`, `build_project` |
| `dev.rs` | The dev-mode watch-and-rebuild loop (covered in [Dev Mode](dev-mode.md)) |
| `devrt.rs` | Building `morph_devrt` via CMake, with source-hash gating |
| `logic.rs` | Per-change logic shared-library compiles (`g++ -shared`) |
| `ipc.rs` | Loopback TCP push to the dev renderer |
| `platform.rs` | OS differences: Linux / macOS / Windows toolchains, GLFW, FreeType/HarfBuzz discovery and bundling |
| `static_deps.rs` | `--static` linking: everything into one self-contained file |
| `upx.rs` | UPX post-processing: squeezing the binary after linking |
| `css_fetch.rs` | Fetching remote stylesheets at build time, so the network never gets consulted at runtime |

## The journey of `morph build`

```
app.cpp + _morph_state.h (+ runtime headers)
    │
    ▼
Compiler (g++/clang++, C++23) — only MORPH_FEATURE_* you actually use
    │
    ▼
fingerprint check — skip what hasn't changed (delete stale binaries when the compiler itself changed)
    │
    ▼
link → .morph/output/<app>
    │
    ├─ --static? → static_deps: bundle everything into one file
    └─ UPX? → upx: compress the binary (skipped with --no-upx)
```

### The compiler driver

`Compiler` invokes the system C++ compiler (`g++-14 -std=c++23` on Linux, clang++ on macOS, MSVC/MinGW on Windows) with the `MORPH_FEATURE_*` defines selected by `morph-codegen`'s `feature_set`. Unused features are compiled out — your calculator does not ship the 3D transform matrix interpolator. This is dead-code elimination with a guest list.

### The fingerprint clerk

Rebuilds skip work whose inputs haven't changed. This is a genuine kindness 99% of the time and a genuine trap the other 1%: when the *compiler itself* changes, old fingerprints can go stale and skip a rebuild you needed. The contributor's reflex for any "but I changed the code and nothing happened" mystery:

```bash
rm -f .morph/output/<name>*
```

Delete the stale binary first, then rebuild. Fingerprinting skips recompiles when only the compiler changed — the docs at the repo root say so, the clerk insists, and now you know the override.

### The dietician (UPX) and the strongman (`--static`)

- **UPX** (`upx.rs`) compresses the linked binary. Smaller downloads, same program. Pass `--no-upx` during development — compression is for shipping, not for iterating.
- **`--static`** (`static_deps.rs`) links everything into a single self-contained file. No "install these five libraries first" README section. Just the binary, the whole binary, and nothing but the binary.

### The platform diplomat

`platform.rs` handles the fact that Linux, macOS, and Windows have never once agreed on anything: different compilers, different OpenGL/GLFW/FreeType/HarfBuzz stories, different bundling rules. `morph doctor` verifies the local toolchain; when a user reports "build fails on machine X", the diplomat's code is where OS-specific assumptions live.

## Concept to pocket: build-time vs runtime

Morph pushes every decidable thing to build time: style resolution, Tailwind lowering, feature selection, CSS fetching. The runtime applies, interpolates, and draws — it never downloads, never negotiates specificity, never wonders what `.btn` means. That split is *the* reason binaries stay under a megabyte and startup stays instant. Every time you are tempted to add runtime negotiation for something knowable at compile time, remember the megabyte and step away.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Change compile flags or the invoked compiler | `lib.rs` (`Compiler`, `BuildOptions`) |
| Change what gets statically linked | `static_deps.rs` |
| Change compression behavior | `upx.rs` |
| Fix an OS-specific build failure | `platform.rs` — and reproduce on that OS, not yours |
| Change dev-renderer rebuild detection | `devrt.rs` source-hash logic |
| Change remote-stylesheet fetching | `css_fetch.rs` |

## Verify by

```bash
cargo test --workspace
rm -f .morph/output/<name>* && <repo>/target/debug/morph build --no-upx
<binary> --morph-self-test     # must report 0 failures
./tests/runtime/run-selftests.sh
```

Rebuild affected fixtures and run them. The C++ compiler is the linter for this layer — if it compiles and the self-tests pass, your change survived contact with reality.
