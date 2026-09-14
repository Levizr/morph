# GUI Pipeline

**Part of:** [Dev Docs](overview.md)

How `morph build` / `morph run` / `morph dev` turn an `.mx` source file into a native windowed binary. Everything here is Rust — there is no Python in the pipeline.

## Build flow

```
.mx source
   │
   ▼
┌──────────────┐   morph_parser: Oxc + lightningcss → JSX walker → MxSource
│ Parse + CSS  │   (imports, components, state, effects, CSS rules + keyframes)
   │
   ▼
┌──────────────┐   morph_ir::IRBuilder: MxSource + CSS → IRWindow/IRNode trees
│ IR           │   (style resolution, Tailwind utilities, transforms)
   │
   ▼
┌──────────────┐   morph_codegen: node_emitter + logic_emitter + feature_set
│ Codegen      │   → app.cpp + _morph_state.h (only used runtime features)
   │
   ▼
┌──────────────┐   morph_build: g++/clang++ (C++23), fingerprinting,
│ Compile      │   --static / UPX post-processing
   │
   ▼
.morph/output/<app>   native binary
```

All three stages run in-process in the `morph` binary. The compiler pipeline crates see:

| Stage | Crate/entry point |
|---|---|
| Parse + CSS + JSX walk | `morph-parser` (`parse_mx_str`, `parse_css`, JSX walker) |
| IR build | `morph-ir::builder::IRBuilder` |
| Emit C++ | `morph-codegen::node_emitter` / `logic_emitter` / `feature_set` |
| Compile + link | `morph-build` (`Compiler`, `BuildOptions`, `build_project`) |
| Runtime linkage | `morph-cache` (`download_runtime`, `link_runtime_to_project`) |
| Config | `morph-config` (output dir, window, build flags) |

## Dev flow

`morph dev` runs the same in-process pipeline, then:

1. Ensures the runtime is installed and builds `morph_devrt` (the prebuilt dev renderer) via CMake if its source hash changed
2. Launches `morph_devrt`, which announces its IPC address
3. Watches source dirs with `notify` (100ms debounce)
4. On change: parse → CSS → IR → emit + compile the logic library → serialize IR → push over **loopback TCP** (`127.0.0.1:39573`, ephemeral fallback on collision) to `morph_devrt`
5. The running window swaps the IR document and rewires logic without restarting (hot reload)

The hot-reload "logic" is compiled per-change (`g++ -shared`) so the window, GL context, and layout tree stay alive.