# Config, Cache & Versions: The Boring Bits That Save You

**Part of:** [Dev Docs](../architecture/overview.md)

Every exciting framework is held up by boring infrastructure it hopes you never think about. This page thinks about it for you: how a project describes itself (`morph-config`), where downloaded runtimes live (`morph-cache`), and how releases know what changed (`versions/`). Boring? Yes. Load-bearing? Completely.

## `morph-config`: the project's ID card

The `morph-config` crate (`crates/morph-config/src/lib.rs`) handles `morph.config.json`, `morph.lock`, and the version files. The config schema, in plain terms:

- `name` — what the thing is called (please, something more descriptive than `test`; the `ui-test` fixture did not get this memo).
- `entry` — the root `.mx` file, usually `src/App.mx`.
- `window` — `width`, `height`, `title`, plus optional min/max constraints (the calculator example uses these to stay pocket-sized at 340×560).
- `renderer` — `"flash"` (production default, recommended) or `"forge"` (beta tile compositor). Only `my-app` dares to run forge, and `my-app` is a playground, not a promise.
- `build` — `wayland`, `system_freetype`, `upx`, `upx_version` knobs.
- `native` — optional interop block: `include_dirs`, `library_dirs`, `libraries`, `cflags`, `ldflags` (the `native-interop` fixture exercises all of these).
- `dependencies`, `cpp_sources` — what else gets compiled in.

`morph.lock` pins the resolved runtime (`cpp 0.1.0`, hash) and which `morphc` generated the project — the "it worked on my machine" antidote.

## `morph-cache`: the global pantry

Downloading the C++ runtime on every build would be madness, so `morph-cache` (`crates/morph-cache/src/lib.rs`) keeps a global pantry at `~/.morph/cache/runtimes/` and links the right version into each project (`download_runtime`, `link_runtime_to_project`).

Two details that bite:

1. **Includes become absolute at emit time.** Morpher rewrites `../../runtime/cpp/...` into the global cache path (`~/.morph/cache/runtimes/cpp/vX.Y.Z/...`) so generated files compile from any directory. New headers under `runtime/cpp/` are picked up automatically — no registration step, no ceremony.
2. **Stale caches cause ghost bugs.** If a build behaves like it is using last month's runtime, it may literally be doing that. `morph cache` inspects and prunes; when in doubt, clear it and rebuild. The pantry is a cache, not an archive — treat it accordingly.

## `versions/`: how releases know what changed

Three small JSON files drive the release machinery:

| File | Current | Meaning |
|---|---|---|
| `versions/morphc/version.json` | `0.1.0` | The toolchain release; `morph update` reads this |
| `versions/runtime/cpp.json` | `0.1.0` | The C++ runtime release |
| `versions/runtime/rust.json` | `0.0.1` | Placeholder for a future Rust runtime |

Each file is `{"version", "changelog", "breaking"}` — enforced by `VersionFile` in `morph-config`. The release workflow (`.github/workflows/release.yml`) detects *which* file changed and ships only that artifact. Bump the wrong file and you cut a release for a component you didn't touch; bump with `breaking: true` and you tell every downstream project to pay attention.

## Concept to pocket: fingerprints, locks, and versions are one idea

Three mechanisms, one philosophy — *never redo work you can prove is fresh, and never trust work you can't prove fresh*:

- Fingerprints skip unchanged recompiles ([The Build Machine](../architecture/build-system.md)).
- `morph.lock` pins the exact runtime a project resolved.
- Version files gate what `morph update` considers new.

When something is stale, the fix is always the same shape: delete the proof of freshness (`rm -f .morph/output/*`, prune the cache, re-resolve the lock) and let the machine re-derive it.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Add a config key | `morph-config` schema + the CLI docs for that key + a fixture exercising it |
| Change cache layout or download logic | `morph-cache` (`download_runtime`, `link_runtime_to_project`) |
| Cut a release | Edit the right `versions/` file; the workflow does the rest |
| Debug "wrong runtime" ghosts | `morph cache`, then the include-rewrite path in morpher's emit |

## Verify by

```bash
cargo test --workspace
python3 -c "import json; json.load(open('docs/dev.registry.json'))"   # registries stay valid JSON
target/debug/morph doctor
```

Config changes deserve a fixture project that exercises the new key — unexercised config is a rumor, not a feature.
