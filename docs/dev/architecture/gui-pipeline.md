# GUI Pipeline

**Part of:** [Dev Docs](overview.md)

How `morph build` / `morph run` / `morph dev` turn `.mx` source into a native windowed binary. Everything here is Rust — there is no Python in the pipeline, only Rust telling `g++` what to do. This is the short version with the full map of who owns what; the scenic route (one component, every stage, worked example) is [Compiler Pipeline](compiler-pipeline.md), and the per-crate deep dives are [crate-parser](../crates/crate-parser.md), [crate-ir](../crates/crate-ir.md), and [crate-codegen](../crates/crate-codegen.md).

## Build flow

```
.mx source
   │
   ▼
┌──────────────┐   morph-parser: Oxc + lightningcss → MxWalker → MxSource
│ Parse + CSS  │   (imports, components, state, effects, CSS rules + keyframes)
│              │   ModuleGraph: BFS over .mx/.ts/.tsx (missing = hard error)
│              │   linter: check → lint → lint_graph (mx-* scope codes)
   │
   ▼
┌──────────────┐   morph-ir::IRBuilder: MxSource + CSS → IRWindow/IRNode trees
│ IR           │   (namespaces, style resolution, Tailwind, transforms,
│              │    state/event identity, emit/on rewrites, serializer)
   │
   ▼
┌──────────────┐   morph-codegen: node_emitter + logic_emitter + feature_set
│ Codegen      │   → app.cpp + _morph_state.h + morph_api.h
│              │   (only used MORPH_FEATURE_*; channels lowered to statics)
   │
   ▼
┌──────────────┐   morph-build: g++/clang++ (C++23), fingerprinting,
│ Compile      │   --static / UPX post-processing
   │
   ▼
.morph/output/<app>   native binary (<1MB, zero runtime deps)
```

All stages run in-process in the `morph` binary. The crates, with their front doors:

| Stage | Crate / entry point | Deep dive |
|---|---|---|
| Parse + CSS + JSX walk | `morph-parser` (`parse_mx_str`, `parse_css`, `MxWalker`) | [crate-parser](../crates/crate-parser.md) |
| Module graph | `morph-parser::resolve` (`resolve_graph`, `module_ns_path`) | [crate-parser](../crates/crate-parser.md) |
| Lint | `morph-parser::linter` (`check`, `lint`, `lint_graph`) | [crate-parser](../crates/crate-parser.md) |
| IR build | `morph-ir::builder::IRBuilder` (`build`, `build_with_graph`) | [crate-ir](../crates/crate-ir.md) |
| Tailwind / transforms / serializer | `morph-ir` (`tailwind.rs`, `transforms.rs`, `serializer.rs`) | [crate-ir](../crates/crate-ir.md) |
| Emit C++ | `morph-codegen` (`CppEmitter::emit`, `node_emitter`, `logic_emitter`) | [crate-codegen](../crates/crate-codegen.md) |
| Feature selection | `morph-codegen::feature_set` (`FeatureSet::scan`) | [crate-codegen](../crates/crate-codegen.md) |
| Compile + link | `morph-build` (`Compiler`, `BuildOptions`, `build_project`) | [Build Machine](build-system.md) |
| Runtime linkage | `morph-cache` (`download_runtime`, `link_runtime_to_project`) | [Config, Cache & Versions](../build-cli/config-cache-versions.md) |
| Config | `morph-config` (output dir, window, build flags) | [Config, Cache & Versions](../build-cli/config-cache-versions.md) |

## What the pipeline refuses to do

The pipeline's character is defined by its refusals, and every refusal is deliberate:

- **No silent `undefined`.** Missing imports are hard errors at graph time (`resolve_graph` bails naming the importer). A typo fails in milliseconds with a filename attached, instead of failing at runtime in front of a user.
- **No string-key identity.** State and event identity is canonical module path + binding name — imports are the registry. (Full machinery: [State & Event Internals](../state/state-events-internals.md).)
- **No runtime negotiation for compile-time facts.** Styles resolve, Tailwind lowers, features select, channels lower to statics — all before `g++` runs. The runtime applies and interpolates; it never wonders what `.btn` means.
- **No interpreter in the output.** Your source files never ship. Only the compiled binary does — which is why the binary stays tiny and startup stays instant.

## Dev flow

`morph dev` runs the same in-process pipeline, then swaps the ending:

1. Ensures the runtime is installed and builds `morph_devrt` (the prebuilt dev renderer) via CMake if its source hash changed
2. Launches `morph_devrt`, which announces its IPC address
3. Watches source dirs with `notify` (100ms debounce — editors save in bursts, rebuild once)
4. On change: parse → CSS → IR → emit + compile the logic library (`g++ -shared`) → serialize IR → push over **loopback TCP** (`127.0.0.1:39573`, ephemeral fallback on collision) to `morph_devrt`
5. The running window swaps the IR document and rewires logic without restarting: signals preserved via the signal store, nodes matched via the node registry, subscriptions cleared and re-registered

The hot-reload "logic" is compiled per-change so the window, GL context, and layout tree stay alive. Only your mistakes get replaced. Full doctrine: [Dev Mode](dev-mode.md).

## Debugging the pipeline: follow the smell upstream

Data flows one way, and so do bugs. Check the *earliest* stage whose output looks wrong:

| Symptom | First suspect | Verify with |
|---|---|---|
| Unknown import / missing component | `resolve.rs` module graph | The import spelling; missing targets are hard errors by design |
| Scope misuse compiles but misbehaves | `linter.rs` scope codes | `morph check` should flag it — a miss means the lint has a hole |
| Wrong styles, right structure | IR style resolution / Tailwind / transforms | Inspect the IR: wrong IR means the renderer is innocent |
| Stale names / bad accessors | `seed_module_bindings`, `state_map` | IR-shape tests asserting exact `app::…` strings |
| Dead emits / double-fire handlers | `rewrite_event_emits` / `translate_event_sub` | The "lambda only" contract in [State & Events](../state/state-events-internals.md) |
| Bloated or under-featured binary | `feature_set.rs` | Which `MORPH_FEATURE_*` the build actually passed |
| "Changed code, nothing happened" | Fingerprint staleness | `rm -f .morph/output/<name>*`, rebuild ([Build Machine](build-system.md)) |

## Verify by

```bash
cargo test -p morph-parser -p morph-ir -p morph-codegen   # the pipeline trio
cargo test --workspace                                     # the full orchestra
rm -f .morph/output/<name>* && <repo>/target/debug/morph build --no-upx
<binary> --morph-self-test && ./tests/runtime/run-selftests.sh
```

Reproduce at the earliest stage, fix at the earliest stage, and prove it with fixtures: the IR-shape tests for structure, the translate fixtures for behavior, the runtime self-tests for the whole breathing app.
