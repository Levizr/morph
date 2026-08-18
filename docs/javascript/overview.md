# JavaScript / TypeScript Overview

Morph compiles your component logic from TypeScript/JavaScript to C++ at build time. There is no interpreter — your code runs as native machine code.

## How It Works

When you write:

```tsx
const [count, setCount] = morphState(0)
```

Morph's TS→C++ translator (`morph/js/`) converts this to equivalent C++ using tree-sitter. The translated code uses Morph's native runtime types (`JsValue`, `JsString`, `Signal<T>`, etc.) and runs directly in the compiled binary.

In **dev mode**, the translated logic is compiled to a shared library (`logic.<hash>.so`) loaded via `dlopen`. Hot reload re-wires signals and effects in place without restarting the window.

In **build mode**, the translated logic is compiled directly into the production binary.

## Importing the Morph Module

Every `.mx` file starts by importing from `morph`:

```tsx
import { CSS, morphState, morphEffect } from 'morph'
```

This module is provided by `node_modules/morph/index.d.ts` — it ships with every `morph init` project and gives your editor autocomplete and type checking.

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
|---|---|
| `int`, `int32`, `int64` | `int` / `int64_t` |
| `float`, `double` | `float` / `double` |
| `string` | `JsString` |
| `boolean` | `JsBoolean` |
| `any` | `JsValue` |
| `MouseEvent` | `MorphEvent*` |
| `Element`, `HTMLElement` | `MorphNode*` |
| `Promise<T>` | `auto` |

### Statements

`if`/`else`, `while`, `for`, `do-while`, `switch`/`case`/`default`, `try`/`catch`/`throw`, `return`, `break`, `continue`

### Expressions

Binary (`+`, `-`, `*`, `/`, `===`, `!==`, `==`, `!=`, `<`, `>`, etc.), unary (`!`, `-`, `++`, `--`), ternary (`? :`), template literals, array/object literals, `new`, member access, function calls, `await`

### JS Runtime Semantics

- `typeof` operator
- Truthiness (same rules as JS)
- `==` / `!=` with JS coercion rules
- String concatenation (`"" + x`)
- Array `push`/`pop`/index access
- Object `has`/`keys`/index access
- String methods: `split`, `trim`, `toUpperCase`, `toLowerCase`, `indexOf`, `substring`, `slice`, `replace`, `charAt`

## What's NOT Supported Yet

- Destructuring assignments
- Spread/rest operators in all contexts
- `import` from other `.mx` files (use CSS.load or C++ imports)
- `class` extends across files
- Generics beyond basic usage
- `async`/`await` in non-event-handler contexts
