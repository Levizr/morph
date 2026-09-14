# morph-parser

Morph source parsing via Oxc + lightningcss — `.mx` / `.mx3` / `.tsx` files to a typed AST.

Part of the [Morph](https://github.com/Levizr/morph) toolchain. Runs an Oxc walker over the JS inside an `.mx` file and lightningcss over its CSS, producing a single typed `MxSource` structure plus applicable lint diagnostics.

## Install

```bash
cargo add morph-parser
```

## Library Usage

```rust
use morph_parser::parse_mx_file;

let source = parse_mx_file("src/App.mx")?;

// Trees of JsxNode (div, text, image, button, input, etc.)
// plus CssData, imports (JSX/component/C++ nodes), state, effects,
// inner functions, component consts, and window config
for component in &source.components {
    println!("{}", component.name);
}
```

## What It Covers

- **Parsing** — `parse_mx_file(path)` and `parse_mx_str(source, filename)`
- **JS walker** — JSX trees, components, event props, imports, `morphState` / `morphEffect`, inner functions, C++ node imports
- **CSS parser** — rules, properties, keyframes, `@font-face` via lightningcss
- **Linter** — `check()` / `lint()` return `LintError` diagnostics with codes and fixes; `suggest_tag()` for typo suggestions

## License

Apache-2.0