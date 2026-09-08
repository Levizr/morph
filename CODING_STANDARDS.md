# Morph Coding Standards

> **Mandatory** coding conventions for Rust and C++ in the Morph project.
> These rules keep the codebase consistent, readable, and maintainable.

---

## Table of Contents

1. [General Principles](#1-general-principles)
2. [Rust Standards](#2-rust-standards)
3. [C++ Standards](#3-c-standards)
4. [Cross-Language Conventions](#4-cross-language-conventions)
5. [Tooling](#5-tooling)
6. [Code Review Checklist](#6-code-review-checklist)

---

## 1. General Principles

| Principle | Rule |
|-----------|------|
| **Consistency > Preference** | Follow the project style even if you disagree. When in doubt, match the surrounding code. |
| **Readability > Cleverness** | Prefer explicit, verbose code over terse "clever" code. |
| **Max Nesting** | 3-4 levels deep. Extract early. |
| **No Comments Unless Asked** | Code should be self-documenting. Comments only for "why", never "what". |
| **No Dead Code** | Remove unused code, don't comment it out. |
| **Handle Errors** | No silent failures. No `unwrap()` in production. No swallowing exceptions. |

---

## 2. Rust Standards

### 2.1 Naming Conventions

| Item | Convention | Example |
|------|-----------|---------|
| Crates | `kebab-case` | `morph-ir`, `morph-codegen` |
| Modules / files | `snake_case.rs` | `node_emitter.rs`, `logic_emitter.rs` |
| Structs / Enums / Traits | `PascalCase` | `IRNode`, `EscapeKind`, `CppTranslator` |
| Functions / Methods | `snake_case` | `emit_expression`, `mark_dirty` |
| Variables | `snake_case` | `var_types`, `cpp_type` |
| Constants / Statics | `SCREAMING_SNAKE_CASE` | `INDENT`, `MAX_RETRIES` |
| Type parameters | Single letter or PascalCase | `T`, `Error`, `Context` |
| Macros | `snake_case!` | `format!`, `vec!` |
| Lifetimes | Single lowercase letter | `'a`, `'src`, `'ctx` |

### 2.2 File Organization

```
crates/
  morphc/src/
    main.rs          # Binary entry — only dispatch + CLI parsing
    commands/        # One file per subcommand (init.rs, dev.rs, build.rs, ...)
    logger.rs        # Shared logging/formatting utilities
    cache.rs
    versions.rs
  morpher/src/
    lib.rs           # Re-exports public API
    parser.rs
    linter.rs
    codegen/
      mod.rs
      analyzer.rs
      cpp.rs         # C++ codegen
      rust.rs        # Rust codegen
      context.rs
      type_resolver.rs
      string_methods.rs
```

**Rules:**
- One struct/enum per file when > 200 lines
- `lib.rs` re-exports only — no logic
- `main.rs` dispatches — no business logic
- Private helper modules with `mod` (not `pub mod`)
- Test modules at bottom: `#[cfg(test)] mod tests { }`

### 2.3 Formatting

Enforced by `rustfmt.toml` (workspace root). Run before every commit:

```bash
cargo fmt --all
```

Key settings:
- **4 spaces**, no tabs
- **100 char** line width
- Braces same line: `fn foo() {`
- Imports grouped: std → external → crate → local
- Vertical struct literals for 3+ fields

### 2.4 Types

```rust
// GOOD: explicit types on public APIs
pub fn translate_program(&mut self, program: &Program<'a>) -> String

// GOOD: Result for fallible operations
fn parse_source(source: &str) -> Result<Program, ParseError>

// GOOD: Option for optional values
fn find_node(&self, id: &str) -> Option<&IRNode>

// BAD: unnecessary type ascription on locals
let x: i32 = 42;  // just let x = 42;

// BAD: &Option<T>
fn process(name: &Option<String>)  // prefer &str or Option<&str>

// BAD: &Vec<T>
fn process(items: &Vec<String>)    // prefer &[String]
```

### 2.5 Error Handling

```rust
// GOOD: thiserror for library errors
#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("unsupported syntax: {0}")]
    UnsupportedSyntax(String),

    #[error("type resolution failed for {name}")]
    TypeResolution { name: String },
}

// GOOD: anyhow for application-level
use anyhow::{Context, Result};

fn load_config(path: &Path) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    Ok(toml::from_str(&content)?)
}

// BAD: unwrap in production code
let value = map.get("key").unwrap();

// BAD: expect with vague message
let value = map.get("key").expect("failed");
```

### 2.6 Imports

```rust
// GOOD: grouped and sorted
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::codegen::context::{Ctx, INDENT};
use crate::codegen::type_resolver::resolve_type;

// BAD: ungrouped
use serde_json::Value;
use crate::codegen::context::Ctx;
use std::collections::HashMap;
use anyhow::Result;
```

### 2.7 Ownership

- Default to stack allocation
- `&T` / `&mut T` for borrowing
- `Cow<'a, str>` for borrowed-or-owned strings
- `Box<T>` only for recursive types, trait objects, or large (>1KB) stack values
- Never use `Rc<RefCell<T>>` — redesign ownership instead
- Minimize `.clone()` — use references

### 2.8 Documentation

```rust
/// Translates a TypeScript program to C++.
///
/// # Arguments
/// * `program` - Parsed AST from oxc
/// * `optimize` - Enable escape analysis
///
/// # Returns
/// Generated C++ source code as a String.
pub fn translate_program(
    &mut self,
    program: &Program<'a>,
) -> String { ... }
```

- Document all public items
- Run `cargo doc` to verify
- Use `///` (not `//!`) except for crate root

### 2.9 Clippy

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Allow list (configured in workspace `Cargo.toml`):
- `too_many_arguments` — codegen functions legitimately need many params
- `module_name_repetitions` — e.g. `IRNode` in `node.rs` is fine
- `must_use_candidate` — not every getter needs `#[must_use]`
- `doc_markdown` — our naming differs from rustdoc expectations

---

## 3. C++ Standards

### 3.1 Naming Conventions

| Item | Convention | Example |
|------|-----------|---------|
| Files | `snake_case.h / .cpp` | `node.h`, `js_value.h`, `layout.cpp` |
| Classes / Structs | `PascalCase` | `MorphNode`, `MorphStyle`, `HoverTransition` |
| Functions / Methods | `snake_case` | `hit_test`, `mark_dirty`, `dispatch_event` |
| Member variables | `m_snakeCase` | `m_dirtyFlags`, `m_paintOrder` |
| Static members | `s_snake_case` | `s_lastHoveredNode`, `s_focusedNode` |
| Local variables | `snake_case` | `child_inv`, `old_scroll_y` |
| Constants (enum) | `PascalCase` or `SCREAMING_SNAKE` | `Clean`, `StyleDirty` |
| Macros / defines | `SCREAMING_SNAKE_CASE` | `MORPH_FEATURE_FLEX` |
| Namespaces | `snake_case` | `morph`, `morph::render` |
| Template params | `PascalCase` | `typename T`, `size_t N` |

### 3.2 File Organization

```
runtime/cpp/
  core/
    node.h              # class declaration + inline small methods
    node.cpp            # major method definitions
    node/
      layout.cpp        # large methods split into feature files
      events.cpp
      style.cpp
      animation.cpp
      flatten.cpp
      paint_order.cpp
    window.h / window.cpp
    compositor.h / compositor.cpp
  render/
    gl_renderer.h / gl_renderer.cpp
  renderers/
    flash/flash.h, flash.cpp
    forge/forge.h, forge.cpp, damage.h, damage.cpp
  reactivity/
    signal.h
    effect.h / effect.cpp
    task.h / task.cpp
  types/
    js_types.h          # umbrella include
    js_value.h
    js_string.h
    js_number.h
    js_array.h
    js_object.h
    js_boolean.h
  style/
    style.h
    features/base.h, flex.h, border.h, ...
  net/
    net.h / net.cpp
  vendor/               # third-party — no formatting changes
```

**Rules:**
- One class per `.h` file (when > 200 lines)
- `.cpp` implements the `.h` of the same name
- Feature-specific code goes in `feature/` subdirectories
- Large classes split into multiple `.cpp` files under a subdirectory
- `#pragma once` (no `#ifndef` guards)
- Header includes: system → project → local

### 3.3 Formatting

Enforced by `.clang-format` (workspace root). Run before every commit:

```bash
clang-format -i <file>
```

Key settings:
- **4 spaces**, no tabs
- **100 char** line width
- **Allman braces** (opening brace on new line):
  ```cpp
  void MorphNode::layout(float px, float py, ...)
  {
      if (scrollEnabled)
      {
          scrollY -= e.scroll * 40.0f;
      }
  }
  ```
- Space after control keywords: `if (`, `while (`, `for (`
- Pointer binds left: `float* ptr`
- One statement per line
- No bin-packing arguments (one per line when wrapping)

### 3.4 Headers

```cpp
#pragma once
#include <vector>
#include <string>

// Forward declarations preferred over heavy includes
class Renderer;

namespace morph {

// Constants — inline constexpr, NOT #define
inline constexpr float DEFAULT_FONT_SIZE = 16.0f;
inline constexpr uint32_t MAX_NODES = 10000;

enum class AnimProperty : uint8_t {
    X, Y, W, H,
    BgColorR, BgColorG, BgColorB, BgColorA,
};

class MorphNode {
public:
    // Public API first
    virtual void layout(float px, float py, float parentW, float parentH, Renderer* r = nullptr);
    virtual void draw(Renderer& r) = 0;

    // Inline short methods
    bool isDirty(uint8_t f) const { return (m_dirtyFlags & f) != 0; }

protected:
    virtual void onHover(bool state);

private:
    // Members last, with m_ prefix
    float x_ = 0, y_ = 0, w_ = 0, h_ = 0;
    uint8_t m_dirtyFlags = Clean;
    std::string nodeId;
    MorphNode* parent = nullptr;
    std::vector<MorphNode*> children;
};

} // namespace morph
```

**Rules:**
- `#pragma once`
- Include `<algorithm>`, `<string>`, `<vector>` by default
- Forward-declare when only pointer/reference is needed
- All code in `namespace morph { }`
- `inline constexpr` for constants (never `#define` for values)
- Access order: `public` → `protected` → `private`
- Short methods (< 5 lines) inline in header
- Longer methods: declare in header, define in `.cpp`

### 3.5 Source Files

```cpp
#include "node.h"
#include "renderer.h"
#include <cmath>
#include <algorithm>

namespace morph {

MorphNode* MorphNode::s_lastHoveredNode = nullptr;

void MorphNode::markDirty(uint8_t f)
{
    m_dirtyFlags |= f;
    if (parent) parent->markDirty(f);
}

} // namespace morph
```

- Own header first, then project, then system
- `using namespace std;` — never in headers, avoid in `.cpp`
- Close every namespace with `// namespace morph`

### 3.6 Modern C++ (C++20)

```cpp
// GOOD: smart pointers
std::unique_ptr<MorphNode> node = std::make_unique<MorphNode>();
std::shared_ptr<JsArray> arr = std::make_shared<JsArray>();

// GOOD: value semantics for small types
struct Vec2 { float x, y; };
void set_position(Vec2 pos);  // pass by value

// GOOD: structured bindings
for (const auto& [key, value] : properties) { ... }

// GOOD: range-based for
for (auto* child : children) { ... }

// GOOD: std::variant for sum types
using JsValue = std::variant<JsUndefined, JsNull, JsNumber, ...>;
std::visit(overloaded{ ... }, value);

// BAD: raw new/delete
MorphNode* node = new MorphNode();
delete node;

// BAD: C-style casts
float f = (float)int_val;  // use static_cast<float>(int_val)

// BAD: #define for constants
#define MAX_NODES 10000     // use inline constexpr
```

### 3.7 Memory Management

| Scenario | Use |
|----------|-----|
| Single ownership, stack-lifetime | Value on stack |
| Single ownership, heap-lifetime | `std::unique_ptr<T>` |
| Shared ownership (callbacks, caches) | `std::shared_ptr<T>` |
| Breaking reference cycles | `std::weak_ptr<T>` |
| Non-owning pointer | Raw `T*` (document lifetime) |

- Never manually `new`/`delete` in new code
- Factory functions return `std::unique_ptr<T>`
- Destructors clean up `shared_ptr`/`unique_ptr` — no manual `delete`

### 3.8 Feature Flags

```cpp
// MORPH_FEATURE_* — compile-time feature gating
#ifdef MORPH_FEATURE_TRANSFORM
    // transform code here
#endif

// In CMakeLists.txt:
target_compile_definitions(morph_devrt PRIVATE
    MORPH_FEATURE_SCROLL
    MORPH_FEATURE_TRANSFORM
    MORPH_FEATURE_ANIMATION
    ...
)
```

- Always wrap feature-specific code in `#ifdef`
- Feature flags defined in CMakeLists.txt
- Dev mode: all features enabled
- Production: only used features (dead code elimination)

### 3.9 Error Handling

```cpp
// GOOD: exceptions for truly exceptional cases
void loadTexture(const std::string& path)
{
    if (!std::filesystem::exists(path))
        throw std::runtime_error("Texture not found: " + path);
}

// GOOD: std::optional for nullable returns
std::optional<MorphNode*> findChild(const std::string& id) const
{
    for (auto* child : children)
        if (child->nodeId == id) return child;
    return std::nullopt;
}

// BAD: returning nullptr as "not found" (ambiguous with "found null")
MorphNode* findChild(const std::string& id);  // unclear semantics
```

---

## 4. Cross-Language Conventions

### 4.1 Type Mapping (Codegen)

| TypeScript | Rust | C++ |
|-----------|------|-----|
| `number` | `f64` / `i32` | `double` / `int32_t` / `JsNumber` |
| `string` | `String` | `std::string` / `JsString` |
| `boolean` | `bool` | `bool` / `JsBoolean` |
| `Array<T>` | `Vec<T>` | `std::vector<T>` / `JsArray` |
| `object` | `HashMap<String, T>` | `std::map` / `JsObject` |
| `Promise<T>` | `impl Future<Output=T>` | `morph::Task<T>` / `morph::Result<T>` |
| `null` | `Option::None` | `JsNull{}` |
| `undefined` | — | `JsUndefined{}` |
| `void` | `()` | `void` |
| `any` | `serde_json::Value` | `JsValue` |

### 4.2 Naming Translation

Codegen translates TypeScript identifiers to target language style:

```
TypeScript    →  Rust (snake_case)    →  C++ (snake_case)
─────────────────────────────────────────────────────────
myVariable       my_variable            my_variable
doSomething()    do_something()         do_something()
MyClass          MyClass                MyClass
MAX_SIZE         MAX_SIZE               MAX_SIZE
```

### 4.3 Shared Concepts

| Concept | Rust (`morph-ir`) | C++ (`runtime/cpp`) |
|---------|-------------------|---------------------|
| Node | `IRNode` | `MorphNode` |
| Style | `IRStyle` | `MorphStyle` |
| Event | `IREvent` | `MorphEvent` |
| Color | `[f32; 4]` | `float[4]` / inline |
| Animation | `IRAnimation` | `MorphAnimation` |
| Node ID | `String` (`node_id`) | `std::string` (`nodeId`) |

---

## 5. Tooling

### 5.1 Rust

| Tool | Command | Config |
|------|---------|--------|
| Formatter | `cargo fmt --all` | `rustfmt.toml` (workspace root) |
| Linter | `cargo clippy --all-targets --all-features -- -D warnings` | `[workspace.lints.clippy]` in `Cargo.toml` |
| Doc | `cargo doc --workspace --no-deps` | |
| Test | `cargo test --all` | |

### 5.2 C++

| Tool | Command | Config |
|------|---------|--------|
| Formatter | `clang-format -i <file>` | `.clang-format` (workspace root) |
| Linter | `clang-tidy <file> -- -std=c++20` | `.clang-tidy` (optional) |
| Build | `cmake -B build && cmake --build build` | `CMakeLists.txt` |

### 5.3 Pre-commit Check

```bash
# Run before pushing
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cd runtime/cpp && find . -name "*.h" -o -name "*.cpp" | grep -v vendor | xargs clang-format --dry-run --Werror
```

### 5.4 Adding CI (`.github/workflows/lint.yml`)

```yaml
name: Lint
on: [push, pull_request]
jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt --all -- --check
      - run: cargo clippy --all-targets --all-features -- -D warnings

  cpp:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: sudo apt-get install -y clang-format
      - run: |
          cd runtime/cpp
          find . \( -name "*.h" -o -name "*.cpp" \) ! -path "*/vendor/*" | xargs clang-format --dry-run --Werror
```

---

## 6. Code Review Checklist

### Rust PR
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --all` passes
- [ ] Public APIs documented
- [ ] No `unwrap()` / `expect()` in production paths
- [ ] No unnecessary `.clone()` — use references
- [ ] Imports grouped and sorted
- [ ] Error types use `thiserror` (libraries) or `anyhow` (applications)

### C++ PR
- [ ] `clang-format --dry-run --Werror` passes
- [ ] Compiles with `-Wall -Wextra -Wpedantic -Werror`
- [ ] No raw `new` / `delete` — smart pointers only
- [ ] No `using namespace` in headers
- [ ] Member variables: `m_` prefix
- [ ] Constants: `inline constexpr`, not `#define`
- [ ] Allman brace style
- [ ] ≤ 100 char line width
- [ ] Feature-specific code wrapped in `#ifdef MORPH_FEATURE_*`

### General
- [ ] Changes are minimal and focused
- [ ] Tests added for new functionality
- [ ] No commented-out code
- [ ] No `TODO` / `FIXME` without issue reference

---

## 7. Migration Plan

1. **Add tooling configs** — `rustfmt.toml`, `.clang-format`, `Cargo.toml` lints *(done)*
2. **Format entire codebase once** — `cargo fmt --all` + `clang-format -i` on all files
3. **Fix critical clippy warnings** — focus on `error` and `warning` level
4. **Add CI checks** — enforce formatting on PR
5. **Document exceptions** — use `#[allow(...)]` with issue link, or `// NOLINT`

```bash
# Step 2: format everything
cargo fmt --all
find runtime/cpp \( -name "*.h" -o -name "*.cpp" \) ! -path "*/vendor/*" -exec clang-format -i {} +
```

---

*Version: 1.0 — 2026*
