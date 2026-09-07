# morpher

**morpher** is the intent-based TypeScript-to-C++/Rust code generator for the Morph toolchain. It translates strict TypeScript (.ts) directly to native C++ or Rust using Oxc parser and semantic analysis.

## Features

- **Direct TypeScript parsing** - Uses Oxc for fast, accurate TS parsing
- **Intent-based codegen** - Escape analysis, type widening, and async boundary detection
- **Smart type inference** - Chooses native C++ types (`int`, `std::string`, `bool`) over JS types (`JsNumber`, `JsString`, `JsBoolean`) when safe
- **Two type modes**:
  - `--type infer` (default) - Analyzes code usage to pick optimal native types
  - `--type strict` - Respects user-declared type annotations
- **Optimized codegen** - `--optimize` enables escape analysis for stack allocation, unique_ptr/shared_ptr for ownership
- **Global runtime includes** - Generates absolute paths to Morph C++ runtime for portable compilation

## Installation

```bash
cargo build --release -p morpher
```

## Usage (Library)

```rust
use morpher::{translate, TranslateOptions, TypeMode};

// Simple translation
let cpp = translate("let x: number = 42;", "file.ts", TranslateOptions::default())?;

// With options
let options = TranslateOptions {
    optimize: true,
    type_mode: TypeMode::Infer,
    runtime_path: Some("/home/user/.morph/cache/runtimes/cpp/v0.1.0".to_string()),
    indent: 0,
};
let cpp = translate(source, "file.ts", options)?;
```

## Usage (CLI via morphc)

```bash
# Default: infer mode, no optimization
morph app.ts --to cpp

# Explicit infer mode with optimization
morph app.ts --to cpp --type infer --optimize

# Strict mode (respect user annotations)
morph app.ts --to cpp --type strict

# Rust target
morph app.ts --to rust
```

## Type Mode Behavior

### `--type infer` (default)
Analyzes how variables are used and chooses the best native C++ type:
- `let num: number = 42` → `int32_t num = 42` (only used for printing)
- `let str: string = "hello"` → `std::string str = "hello"`
- `let flag: boolean = true` → `bool flag = true`
- `let any: any = "value"` → `std::string any = "value"` (inferred from init)
- Unannotated vars always inferred from init value

Variables that escape (returned, captured in closures, cross async boundaries) get appropriate smart pointers (`unique_ptr`, `shared_ptr`).

### `--type strict`
Respects user type annotations:
- `let num: number = 42` → `JsNumber num = 42` (annotation preserved)
- `let str: string = "hello"` → `JsString str = "hello"`
- Unannotated vars inferred from init value

## Architecture

```
src/
├── lib.rs              - Public API (translate, TranslateOptions, TypeMode)
├── parser.rs           - Oxc parsing + entry points
├── codegen/
│   ├── mod.rs          - Re-exports
│   ├── context.rs      - Codegen context (includes, type_mode, runtime_path)
│   ├── type_resolver.rs - TS type → C++ type mapping
│   ├── analyzer.rs     - Escape analysis, type widening, async detection
│   ├── cpp.rs          - C++ emitter (CppTranslator)
│   └── rust.rs         - Rust emitter (RustTranslator)
├── error.rs            - Error types
└── linter.rs           - TS linting rules
```

## Type Resolution

### Infer Mode (default)
| TS Type | Init Value | C++ Type |
|---------|------------|----------|
| `number` | `42` | `int32_t` |
| `number` | `3.14` | `double` |
| `string` | `"hello"` | `std::string` |
| `boolean` | `true` | `bool` |
| `null` | `null` | `JsNull` |
| `undefined` | `undefined` | `JsUndefined` |
| `any` | `"value"` | `std::string` (from init) |
| (unannotated) | `100` | `int32_t` |
| (unannotated) | `"world"` | `std::string` |

### Strict Mode
| TS Type | C++ Type |
|---------|----------|
| `number` | `JsNumber` |
| `string` | `JsString` |
| `boolean` | `JsBoolean` |
| `null` | `JsNull` |
| `undefined` | `JsUndefined` |
| `any` | `JsValue` |
| `object` | `JsObject` |
| (unannotated) | Inferred from init |

## Optimized Codegen (`--optimize`)

When `--optimize` is enabled, the analyzer performs:

1. **Escape Analysis** - Determines if variables escape scope:
   - `None` → Stack allocation
   - `Return`/`Global` → `unique_ptr` + move
   - `ClosureCapture`/`MultipleRefs`/`AsyncBoundary` → `shared_ptr`

2. **Type Widening** - Tracks when native types must widen to JS types:
   - Arithmetic on unknown values → `JsNumber`
   - `.toString()` called → `JsString`
   - Dynamic sources (`fetch`, `JSON.parse`) → `JsNumber`

3. **Async Detection** - Marks functions as async, wraps return in `morph::Result<T>` or `morph::Task`

## Runtime Includes

Generated code uses absolute paths to the Morph C++ runtime:
```cpp
#include "/home/user/.morph/cache/runtimes/cpp/v0.1.0/types/js_types.h"
```

This makes the generated file compilable from any directory without `-I` flags. The runtime path is resolved automatically from `~/.morph/cache/runtimes/cpp/` or the local `runtime/cpp/` directory.

## Requirements

- Rust 1.80+
- C++23 compatible compiler (g++-14, clang-18)
- Oxc parser dependencies (in workspace)

## License

Apache-2.0