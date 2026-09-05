# JavaScript / TypeScript Overview

Morph compiles component logic from TypeScript/JavaScript to C++ at build time. There is no interpreter — code runs as native machine code.

## How It Works

When you write:

```tsx
const [count, setCount] = morphState(0)
```

Morph's TS→C++ translator (the `morph-js` crate) converts this to equivalent C++ using Oxc. The translated code uses Morph's native runtime types (`JsValue`, `JsString`, `Signal<T>`, etc.) and runs directly in the compiled binary.

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

| TypeScript | C++ (Legacy) | C++ (`--optimize`) |
|---|---|---|
| `int`, `int32`, `int64` | `int` / `int64_t` | `int32_t` / `int64_t` (stack) |
| `float`, `double` | `float` / `double` | `float` / `double` (stack) |
| `string` | `JsString` | `std::string` (stack) or `JsString` if dynamic |
| `boolean` | `JsBoolean` | `bool` (stack) |
| `any` | `JsValue` | `JsValue` |
| `MouseEvent` | `MorphEvent*` | `MorphEvent*` |
| `Element`, `HTMLElement` | `MorphNode*` | `MorphNode*` |
| `Promise<T>` | `morph::Result<T>` | `morph::Result<T>` |
| `Promise<void>` | `morph::Task` | `morph::Task` |

### Statements

`if`/`else`, `while`, `for`, `do-while`, `switch`/`case`/`default`, `try`/`catch`/`throw`, `return`, `break`, `continue`

### Expressions

Binary (`+`, `-`, `*`, `/`, `===`, `!==`, `==`, `!=`, `<`, `>`, etc.), unary (`!`, `-`, `++`, `--`), ternary (`? :`), template literals, array/object literals, `new`, member access, function calls, `await`

### JS Runtime Semantics

- Truthiness (same rules as JS)
- `==` / `!=` with JS coercion rules
- String concatenation (`"" + x`)
- Array `push`/`pop`/index access
- Object `has`/`keys`/index access
- String methods: `trim`, `toUpperCase`, `toLowerCase`, `indexOf`, `substring`, `slice`, `replace`, `charAt`
- Console output: `console.log` / `console.warn` / `console.error` / `console.info` print natively (visible in the DevTools **Logs** tab)

## What's Not Supported Yet

- Destructuring assignments
- Spread/rest operators in all contexts
- `import` from other `.mx` files (use `CSS.load` or C++ imports)
- `class` extends across files
- Generics beyond basic usage
- `async`/`await` in non-event-handler contexts
- The `typeof` operator (compare against `undefined`/`null` instead)
- JS-only string/array methods the native types don't implement — `.split()`, `.includes()`, `.startsWith()`, `.map()`, `.filter()`, `.join()`, etc.

The linter (`morph check`) rejects unsupported operators and methods with a clear error before building.

## Two Codegen Modes

| Mode | Command | Behavior |
|---|---|---|
| **Legacy** | `morph file.ts` | `auto` inference, `Js*` types everywhere, no escape analysis |
| **Optimized** | `morph file.ts --optimize` | Intent-based: escape analysis → stack/`unique_ptr`/`shared_ptr`, native types (`int32_t`, `std::string`, `std::vector`), type widening only when needed |

See [Intent-Based Codegen](../guides/intent-based-codegen.md) for the full memory management strategy.