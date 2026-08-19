# Broader TS→C++ Translator Coverage

**Status:** development → future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Closing the gap between "JS you can write" and "JS that compiles to C++". `morph check` already audits unsupported syntax; this page catalogs exactly what's missing and what each piece unlocks.

## Why it matters

The translator (`morph/js/codegen.py`) is the heart of the "write JS, get native binary" promise. Every unsupported operator, builtin, or method is a workaround users hit. Coverage growth is also the top contribution area.

## Currently supported (the baseline)

- Statements: `if`/`while`/`for`/`do-while`/`switch`/`try-catch`/`throw`, variable declarations, functions + arrow functions, classes/interfaces (inheritance, `super`, generics), `return`/`break`/`continue`
- Expressions: binary/unary/ternary/sequence/update/assignment, template literals, array literals **with spread**, object literals, `new`, member access, calls, `await`
- Builtins: `fetch()`, `console.*`, `setTimeout`/`setInterval`/`clearTimeout`, `morphState`/`morphEffect`, `Error`
- Value semantics: truthiness, `==`/`!=`, `typeof`, string concatenation coercion, array `push`/`pop`/index, object `has`/`keys`, string `toUpperCase`/`toLowerCase`/`trim`/`indexOf`/`substring`/`slice`/`replace`/`charAt`

## Missing — operators (currently compiled to `/* comment */` or rejected)

| Construct | Today | Unlocks |
|---|---|---|
| `**` / `**=` | `/* pow */` / error | exponent math |
| `>>>` (unsigned shift) | `/* >>> */` | bit-twiddling code |
| `typeof` | `/* typeof */` | runtime type checks |
| `void` / `delete` | `/* void */` / `/* delete */` | JS idioms |
| `&&=` / `\|\|=` / `??=` | `/* ... */` | shorthand assign |
| `instanceof` / `in` | error | type/containment checks |
| Optional chaining `?.` | error ("null check silently dropped") | safe deep access |
| Generator functions / `yield` | error | lazy sequences |
| `for...in` / `for...of` | error | iteration over keys/values |
| Destructuring (except `morphState` pattern) | error | multi-return unpacking |
| Spread in call args / object spread | error | functional style |

## Missing — globals (`morph check` rejects: `mx-js-global`)

`document`, `window`, `localStorage`, `alert`/`prompt`/`confirm`, `requestAnimationFrame`, `Math`, `JSON`, `Date`, `RegExp`, `Promise`, `Proxy`, `Map`/`Set`/`WeakMap`, `parseInt`, event constructors, `addEventListener`.

Interesting candidates (the ones a native UI framework plausibly implements):

| Global | What it would map to |
|---|---|
| `Math` | native `<cmath>` — trivial, high value |
| `JSON` | the existing JSON DOM parser in dev runtime |
| `Date` | `std::chrono` |
| `parseInt` / `parseFloat` | small wrappers |
| `Map` / `Set` | hash containers |
| `requestAnimationFrame` | `co_await next_frame()` (already exists!) |
| `localStorage` | a config-file-backed store |

## Missing — methods (`mx-js-method`)

State values implement only `{length, push, pop, get, toString}`. Strings implement `{length, empty, toUpperCase, toLowerCase, substring, slice, charAt, indexOf, replace, trim, toString}`.

- **Arrays:** `map`, `filter`, `forEach`, `reduce`, `includes`, `indexOf`, `find`, `join`, `concat`, `sort`
- **Strings:** `split`, `includes`, `startsWith`, `endsWith`, `padStart`, `padEnd`, `repeat`, `match`

`split` and array `map`/`filter`/`forEach`/`includes` are the highest-value additions — they'd cover most real-world UI logic.

## Note on `morph check`

All of the above are already audited with precise diagnostics and suggestions (`mx-js-op`, `mx-js-syntax`, `mx-js-global`, `mx-js-member`, `mx-js-method`), so progress on coverage is measurable: each item disappears from `morph check` output when done.

## Current state

| Piece | State |
|---|---|
| Array spread + `slice` + `.length` semantics | ✅ Shipped (latest cycle) |
| Operator / builtin / method coverage above | ❌ Missing (each item either comments itself out or errors) |
| `morph check` diagnostics for every missing item | ✅ Shipped |

## Suggested order

1. `Math` + `parseInt`/`parseFloat` (trivial, high usage)
2. `Array.map/filter/forEach/includes` (JSX list rendering already does `.map` — the pattern is proven)
3. `String.split/includes/startsWith/endsWith`
4. Optional chaining `?.` (biggest ergonomics win)
5. `Map`/`Set`, `Date`, `JSON`, `requestAnimationFrame`