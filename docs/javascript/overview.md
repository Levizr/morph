# JavaScript / TypeScript Overview

Morph compiles component logic from TypeScript/JavaScript to C++ at build time. There is no interpreter — code runs as native machine code.

## How It Works

When you write:

```tsx
const [count, setCount] = morphState(0)
```

Morph's TS→C++ translator (the `morpher` crate) converts this to equivalent C++ using Oxc. The translated code uses Morph's native runtime types (`JsValue`, `JsString`, `Signal<T>`, etc.) and runs directly in the compiled binary.

In **dev mode**, the translated logic is compiled to a shared library (`logic.<hash>.so`) loaded via `dlopen`. Hot reload re-wires signals and effects in place without restarting the window.

In **build mode**, the translated logic is compiled directly into the production binary.

## Importing the Morph Module

Every `.mx` file starts by importing from `morph`:

```tsx
import { CSS, morphState, morphEffect } from 'morph'
```

This module is provided by `node_modules/morph/index.d.ts` — it ships with every `morph new` project and gives the editor autocomplete and type checking.

## Supported TypeScript Surface

### Declarations

- `const`, `let`, `var` with type annotations
- `function` declarations and expressions
- Arrow functions (`() => {}`)
- Classes with constructors, methods, `super()`, `this`
- Interfaces and type aliases
- `enum` (basic)

### Type Annotations

| TypeScript | C++ |
|---|---|---|
| `int`, `int32`, `int64` | `int32_t` / `int64_t` (stack, proven range) |
| `float`, `double` | `float` / `double` (stack) |
| `string` | `std::string` (stack) or `JsString` if dynamic |
| `boolean` | `bool` (stack) |
| `any` | `JsValue` |
| `MouseEvent` | `MorphEvent*` |
| `Element`, `HTMLElement` | `MorphNode*` |
| `Promise<T>` | `morph::Result<T>` |
| `Promise<void>` | `morph::Task` |

### Statements

`if`/`else`, `while`, `for`, `do-while`, `switch`/`case`/`default`, `try`/`catch`/`throw`, `return`, `break`, `continue`

### Expressions

Binary (`+`, `-`, `*`, `/`, `===`, `!==`, `==`, `!=`, `<`, `>`, etc.), unary (`!`, `-`, `++`, `--`), ternary (`? :`), template literals, array/object literals, `new`, member access, function calls, `await`

### JS Runtime Semantics

- Truthiness (same rules as JS, except empty arrays are falsy)
- `==` / `!=` with JS coercion rules, `===` / `!==`, relational operators — on native and `Js*` types alike (see [How JavaScript Comparisons Work in Morph](./js-comparisons.md))
- String concatenation (`"" + x`)
- Array `push`/`pop`/index access
- Object `has`/`keys`/index access
- String methods on both `JsString` and native `std::string`: `trim`, `toUpperCase`, `toLowerCase`, `indexOf`, `substring`, `slice`, `replace`, `charAt`, plus `split`, `startsWith`, `endsWith`, `includes`, `repeat`, `padStart`, `padEnd` — natives lower to `morph::str::*` helpers, chains nest (see [Native C++ Types](./native-types.md#js-methods-on-natives))
- Number `.toString()` on native `int`/`double` (lowers to `morph::str::to_string`)
- `Array.push` on both `JsArray` (`.push`) and `std::vector` (`.push_back`), `.length` on vectors, arrays, and strings alike (`.size()` where native)
- Console output: `console.log` / `console.warn` / `console.error` / `console.info` print natively (visible in the DevTools **Logs** tab)

## What's Not Supported Yet

- Destructuring assignments
- Spread/rest operators in all contexts
- `import` from other `.mx` files (CSS: `import "./x.css"`; native code: C++ imports)
- `class` extends across files
- Generics beyond basic usage
- `async`/`await` in non-event-handler contexts
- The `typeof` operator (compare against `undefined`/`null` instead)
- JS-only methods the natives don't implement yet — `.map()`, `.filter()`, `.join()`, `.toFixed()`, `.toPrecision()`, etc. (`.split()`, `.includes()`, `.startsWith()` already lower to `morph::str::*` helpers)

The linter (`morph check`) rejects unsupported operators and methods with a clear error before building.

## Codegen Mode

| Aspect | Command | Behavior |
|---|---|---|
| **Default** | `morph file.ts` | Intent-based: escape analysis → stack/`unique_ptr`/`shared_ptr`, native types (`int32_t`, `std::string`, `std::vector`), type widening only when needed |
| **Types** | `morph file.ts --type strict` / `--type infer` | Respect annotations, or infer from code (default) |

Orthogonal to the mode, `--type` controls annotations:

| Flag | Behavior |
|---|---|
| `--type infer` (default) | Ignore annotations, infer the cheapest native type from initializer and usage (trusted number annotations excepted) |
| `--type strict` | Respect annotations (`number` stays `JsNumber`); unannotated variables are still inferred |

```bash
morph app.ts --to cpp --type infer   # native types + escape analysis
morph app.ts --to cpp --type strict  # annotations exactly as written
```

See [Intent-Based Codegen](../guides/intent-based-codegen.md) for the full memory management strategy, and [Native C++ Types](./native-types.md#how---type-picks-native-vs-wrapper) for how the two flags interact.