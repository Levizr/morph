# Broader TS→C++ Translator Coverage

**Status:** development · **Priority:** high

> **Note:** This is the working plan, not a promise. Items move as implementation teaches us what is safe to lower; every shipped item must lose its `morph check` gate and gain a runtime fixture.

Closing the gap between "JS you can write" and "JS that compiles to C++". `morph check` is the contract: a clean check means the translator has a native representation for everything in the file. This page replaces the old feature wishlist with the actual baseline, ranked gaps, and delivery order.

## Why it matters

The translator (`crates/morpher/`) is the heart of the "write JS, get native binary" promise. A missing operator hurts once; a missing array method or object spread blocks every CRUD screen. The goal is not 100% JavaScript or browser parity — it is a small, honest subset that covers ordinary application code without `.cpp` escape hatches.

## Definition of usable

Morph's JS subset is usable when these apps build with **no native helpers**:

1. **CRUD/list app** — filter, sort, map, reduce, object updates, component-body destructuring.
2. **API dashboard** — `fetch`, real `response.json()`, dates, `Promise.all`, a regex validation.
3. **Settings app** — persistent storage, clipboard, next-frame animation, native dialogs.

Until those pass, coverage work is measured against them rather than against the size of the JavaScript spec.

## Current baseline

Already shipped and **not** on the missing list:

- Statements: `if`/`else`, `while`, `for`, `do-while`, `switch`, `try`/`catch`, `throw`, `return`, `break`, `continue`.
- Expressions: binary/unary/ternary/sequence/update/assignment, template literals, array literals with spread, object literals with spread and computed keys, `new`, member access, calls, optional chaining `?.`, nullish coalescing `??`, `await`.
- Functions/classes: declarations, expressions, arrows, default/optional parameters, classes, inheritance, `super`, getters/setters, interfaces, generics.
- Destructuring: declarations, callable/event payloads, defaults, rest elements (object and array), computed keys, nested patterns. `JsObject`/`JsArray` rest use runtime helpers.
- Component-body flow: branches, loops, and calls between declarations lower into the setup effect; reassigned `let` becomes a signal.
- Builtins: `fetch` (`Response.text`/`json`/`arrayBuffer`/`clone`, `ok`, `status`), `console.*`, timers, `Promise` constructor + `await`, `Error`, `morphState`/`morphEffect`/`morphShared`/`morphEvent`.
- Builtin namespaces: `Math` constants plus `min`/`max`, rounding, logs, exponentials, trig, `hypot`, `fround`, and `random`; `JSON.parse`/`stringify`, `Number.isFinite`/`isNaN`/`isInteger`, `Object.keys`/`values`/`entries`, `Array.isArray`, `String`/`Number`/`Boolean` conversions, `parseInt`/`parseFloat`.
- Strings: broad method coverage including `toUpperCase`, `toLowerCase`, `trim*`, `indexOf`, `substring`, `slice`, `replace*`, `split`, `includes`, `startsWith`, `endsWith`, `pad*`, `repeat`, `match*`, `search`, `normalize`.
- Arrays: `push`/`pop`, `join`, `slice`, `concat`, `includes`, `indexOf`, `lastIndexOf`, `at`, `reverse`, `toString`, plus the general methods `map`, `filter`, `forEach`, `reduce`, `find`, `findIndex`, `some`, `every`, `sort`, `splice`, `shift`, `unshift`, `fill`, `flat`, `flatMap` (multi-argument `push` and spread arguments on supported methods included). Receivers that call these lower to `JsArray`; `std::vector` stays the fast path for loops and indexing. A JSX-producing `.map` is still only valid as a direct JSX child.
- Objects: `has`, `keys`, `get`, `values`, `entries`.
- Operators with JS semantics: `==`/`!=`, `===`/`!==`, relational, `in`, `**`/`**=`, `&&=`/`||=`/`??=`, truthiness, string concatenation, `ToNumber` in numeric contexts.

The old version of this page claimed `Math`, `parseInt`, `JSON`, optional chaining, destructuring, and most string methods were missing. They are not. Treat the linter tables and this baseline as the source of truth.

## Gap map

Priority is user impact, not spec size.

| Priority | Gap | Why it blocks apps | Effort |
|---|---|---|---|
| **P0** | General array methods: `map`, `filter`, `forEach`, `reduce`, `find`, `findIndex`, `some`, `every`, `sort`, `splice`, `shift`, `unshift`, `fill`, `flat`, `flatMap` | Every list, search, sort, and CRUD screen. `.map()` works in JSX list lowering and in ordinary logic; JSX-producing `.map` outside a list position stays rejected. | Large |
| **P0** | Fail loudly on silent-wrong forms | General call spread emits `/* spread */` and passes one array argument. Wrong binaries are worse than errors; `morph check` now rejects `f(...args)` and `new C(...args)` while the variadic array helpers stay supported. | Small |
| **P1** | Rest parameters and general call spread | Wrapper functions, `f(...args)`, variadic helpers. Rest *destructuring* works; rest *parameters* do not. | Medium |
| **P1** | Date subset | Timestamps, formatting, timezone-aware display. Examples currently escape to `osDate()`/`osTime()`. Math is no longer a gap. | Medium |
| **P1** | `Promise.resolve`/`all`/`allSettled`/`race` | Multi-request screens and dashboard loading. | Medium |
| **P1** | TypeScript enums | Codegen has an enum emitter; the linter blocks it. Numeric enums first. | Small |
| **P1** | RegExp literals and basic `.test`/`.exec` | Validation and text search. Regex-backed string helpers already exist. | Medium |
| **P1** | `Map` / `Set` | Keyed collections, insertion order, non-string keys. | Medium |
| **P1** | Object/Array statics | `Object.assign`, `Object.fromEntries`, `Array.from`, `Array.of`. | Small–medium |
| **P2** | Platform services: persistent storage, clipboard, next frame, native dialogs | Desktop-app basics. Prefer native Morph APIs; alias browser names only where semantics match. | Medium |
| **P2** | `typeof` | Type-aware validation idioms; needs a strategy that survives native lowering. | Small–medium |
| **P3** | Namespace imports, class expressions, function values in objects | Ergonomics, not blockers. Function-in-object support needs a closure-storage decision. | Small–medium |

### Explicit non-goals

Do **not** chase browser parity:

- DOM (`document`, `window`, `location`, `history`, `navigator`), event constructors, `addEventListener`.
- Full `Intl`, locale/timezone formatting, TypedArrays, `Proxy`, `Reflect`, `WeakMap`, `BigInt`, `Symbol`.
- Generators/`yield`, `with`, labeled jumps, dynamic `import()`, decorators.
- A general `Promise.then` chain runtime — Morph uses `async`/`await` coroutines.

Morph should expose native equivalents (`useWindow`, clipboard, storage) instead of pretending to be a browser.

## Delivery plan

### Milestone 1 — ordinary TS compiles

- [x] `Response.json()` uses the real JSON parser; Response detection includes `json`/`arrayBuffer`/`clone`/`bodyUsed`.
- [x] Object spread in expressions (last key wins, computed keys evaluate once).
- [x] Component-body top-level destructuring (single evaluation, one const per binding, reactivity preserved).
- [x] General array methods lowered on `JsArray`; receivers that use them are forced to `JsArray`; `std::vector` stays the fast path for loops/indexing.
- [x] Multi-argument `push` and spread arguments on supported array methods.
- [x] `morph check` rejects general call spread until it lowers correctly; array methods with variadic helpers (`push`/`unshift`/`splice`/`concat`) and `Math.min`/`Math.max` remain allowed.
- [x] `**`/`**=` (ToNumber semantics through `std::pow`) and `&&=`/`||=`/`??=` for simple targets.
- [ ] Numeric TS enums.

Exit: the CRUD/list reference app builds without a `.cpp` helper.

### Milestone 2 — application standard library

- [ ] Date subset: construction, `now`, `parse`, `getTime`, `toISOString`, calendar/time getters.
- [x] Common `Math` members and constants: `random`, `trunc`, `sign`, logs, exponentials, trig, `hypot`, `fround`, and all eight constants.
- [ ] `Promise.resolve`/`all`/`allSettled`/`race` (correctness first; parallel execution later).
- [ ] RegExp literal plus `.test`/`.exec`; document unsupported regex features.
- [ ] `Map`/`Set` runtime wrappers with JS key semantics and insertion order.
- [ ] `Object.assign`/`fromEntries`, `Array.from`/`of`, remaining safe statics.

Exit: the API dashboard reference app builds without a `.cpp` helper.

### Milestone 3 — platform services

- [ ] Persistent key-value storage (file-backed; optionally aliased to `localStorage`).
- [ ] Clipboard bridge.
- [ ] `requestAnimationFrame` → next-frame coroutine.
- [ ] Native dialogs behind a documented API.

Exit: the settings reference app builds without a `.cpp` helper.

### Milestone 4 — compatibility tail

Only with demonstrated demand:

- [ ] `import * as ns` namespace imports.
- [ ] Class expressions assigned to consts.
- [ ] Function values in object literals (requires a real closure-storage design).
- [ ] `typeof` / `instanceof` runtime narrowing helpers.
- [ ] Lower-priority operators: `delete`, `void`, `>>>`.

## Engineering rules

1. **No silent fallbacks.** A codegen placeholder (`/* spread */`, `/* ... */`) must become either real lowering or a `morph check` error in the same change.
2. **One capability table.** The linter and translator currently duplicate supported-method/builtin lists. Extract shared tables so a feature cannot be half-shipped.
3. **Every feature gets three tests:** runtime helper unit test, translator fixture, and conformance fixture (pass, or fail for rejected edge cases).
4. **Docs ship with the feature.** Remove the item from `docs/javascript/overview.md` "What's Not Supported Yet", update the matching error page, and keep the registry description current.
5. **No browser shims by accident.** A browser-named API is only added when its semantics are honest; otherwise expose a Morph-native name.

## Verification

For every milestone:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
./tests/runtime/check-js-types.sh
./tests/runtime/check-conformance.sh
./tests/runtime/run-selftests.sh
```

Plus a manual build of the reference app and a headless `--morph-self-test` run.

## Current state

| Piece | State |
|---|---|
| Baseline above (excluding gap map) | ✅ Shipped |
| P0 silent-wrong audit (general call spread) | ✅ Shipped — rejected by `mx-js-syntax`; variadic array methods stay allowed |
| `Response.json()` / Response detection | ✅ Shipped |
| Object spread and computed keys | ✅ Shipped |
| Component-body destructuring | ✅ Shipped |
| General array methods | ✅ Shipped |
| `**`/`**=` and `&&=`/`||=`/`??=` | ✅ Shipped — ToNumber via `std::pow`; logical assignments short-circuit on simple targets |
| Milestones 2–4 | Planned |
| `morph check` diagnostics for rejected constructs | ✅ Shipped; grows with each item |
