# morph-codegen

Morph C++ / Rust code generation — IR to native source for runtime and build targets.

Part of the [Morph](https://github.com/Levizr/morph) toolchain. The `CppEmitter` produces C++ for the application window tree and logic (`emit_logic`, `emit_node`), while the `RustEmitter` targets Rust output for file morphing projects.

## Install

```bash
cargo add morph-codegen
```

## Library Usage

```rust
use morph_codegen::{node_emitter::emit_node, logic_emitter::emit_logic};

// Emit a single IR node as C++ (with feature detection)
let code = emit_node(node, None, &features);

// Emit full application logic (state header, node creation, listeners)
let output = emit_logic(&ir_windows);
for source_file in &output.files {
    std::fs::write(&source_file.path, &source_file.content)?;
}
```

## What It Covers

- **`node_emitter`** — `emit_node` / `emit_node_with_state`, keyframe registration, per-node C++
- **`logic_emitter`** — `emit_logic` → `LogicOutput` (files), `generate_state_header`, list-node collection
- **`feature_set`** — `FeatureSet` scanning which runtime features a build needs
- **`cpp` / `rust`** — `CppEmitter` and `RustEmitter` backend entry points

## License

Apache-2.0