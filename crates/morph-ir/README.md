# morph-ir

Morph Intermediate Representation — the typed IR that every renderer and codegen backend consumes.

Part of the [Morph](https://github.com/Levizr/morph) toolchain. `IRBuilder` turns a parsed `MxSource` into `IRNode` trees with resolved style, Tailwind utilities, keyframes, animations, and transforms — the contract between parser, codegen, and the C++ runtime.

## Install

```bash
cargo add morph-ir
```

## Library Usage

```rust
use morph_ir::builder::IRBuilder;

let ir = IRBuilder::new(source).build(&config)?;

// Tree of IRNode with id, tag, style, layout, children, events
for window in &ir.windows {
    for node in window.tree.iter() {
        println!("{} ({:?})", node.id, node.tag);
    }
}
```

## What It Covers

- **`IRBuilder`** — `MxSource` → IR windows/nodes, keyframes, animations, state
- **`IRNode`** — id, tag, text, style, layout, events, conditional class effects, children
- **`IRStyle`** — resolved CSS properties fed to codegen and runtime
- **`css_registry`** — known-property and per-feature lookup (`property_feature`)
- **`tailwind`** — `TailwindResolver` expansion of utility classes
- **`transforms`** — `parse_transform` / `compose_transform` / `parse_transform_origin` for GPU transform matrices
- **`serializer`** — `IRSerializer` for debug / dev-mode output

## License

Apache-2.0