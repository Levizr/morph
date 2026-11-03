# TypeScript App-Logic Coverage

**Status:** development · **Priority:** high

> **Note:** This is the working plan, not a promise. Items move as implementation teaches us what is safe to lower; every shipped item must lose its `morph check` gate and gain a runtime fixture.

Morph should be able to tell a user: **“Write your app logic in TypeScript.”** This page defines what that promise requires, what already works, and the dependency-ordered parts that close the gap.

`morph check` is the contract. A clean check means the translator has a native representation for everything in the file — and that the generated program preserves JavaScript semantics for the accepted subset. If either half is false, the construct belongs in this plan, not in the supported baseline.

## The promise

“App logic” means the code behind a normal desktop application, not browser parity:

- state, events, forms, validation, and conditional rendering;
- lists, filtering, sorting, grouping, and object updates;
- `fetch`, JSON, dates, regular expressions, and error/loading states;
- collections (`Map`/`Set`), string/number helpers, and URL handling;
- persistent settings, clipboard, next-frame animation, and native dialogs.

It does **not** mean the DOM, full npm, full `Intl`, TypedArrays, `Proxy`, generators, or decorators. Those are listed as non-goals below unless a concrete app requirement changes the decision.

The promise is honest only when all three contract rules hold.

## The contract

1. **Clean check ⇒ C++ compiles.** No construct may pass `morph check` and then fail in the generated C++.
2. **Accepted code ⇒ same observable behavior.** No accepted construct may silently change meaning. Wrong binaries are worse than errors.
3. **Every accepted construct is tested.** Each one gets a runtime/translator test and a conformance pass fixture. Each rejected construct gets a precise `mx-*` error and a conformance fail fixture.

Status labels used on this page:

| Label | Meaning |
|---|---|
| ✅ | Shipped and covered by fixtures |
| ⚠️ | `morph check` passes, generated C++ fails |
| 🟥 | `morph check` passes, generated program is silently wrong |
| ⛔ | Rejected by `morph check` with a precise error |
| 🚧 | Not available yet; no native representation |
| 📋 | Documentation/registry work only |

## Definition of done

Morph's TypeScript subset is usable when these apps build with **no native helpers** and no manual memory management:

| Reference app | Must cover |
|---|---|
| **CRUD/list app** | Create/edit/delete, filter/sort/search, object updates, component-body destructuring, form validation, empty/error states. |
| **API dashboard** | `fetch` GET/POST with method/headers/body, real `response.json()`, `Promise.all`, dates, regex validation, loading/error/retry states. |
| **Settings app** | Persistent key-value storage, clipboard, next-frame animation, native dialogs, window handles. |

Until those pass, coverage work is measured against them rather than against the size of the JavaScript spec.

## Current baseline

Already shipped and **not** on the missing list:

- **Statements:** `if`/`else`, `while`, `for`, `do-while`, `switch`, `try`/`catch`, `throw`, `return`, `break`, `continue`.
- **Expressions:** binary/unary/ternary/sequence/update/assignment, template literals, array literals with spread, object literals with spread and computed keys, `new`, member access, calls, optional chaining `?.`, nullish coalescing `??`, `await`.
- **Functions/classes:** declarations, expressions, arrows, default/optional parameters, classes with explicitly typed fields, inheritance, `super`, getters/setters with typed backing fields, interfaces, generics.
- **Destructuring declarations:** callable/event payloads, defaults (`{ a = 1 }`), rest elements (`{ ...rest }`, `[head, ...tail]`), computed keys, and nested patterns. Assignment patterns and loop binding patterns are Part 1C.
- **Component-body flow:** branches, loops, and calls between declarations lower into the setup effect; reassigned `let` becomes a signal; the functional state updater (`setCount(c => c + 1)`) works.
- **Late-assigned values:** `let x; x = 5; x = "hi"` works through `JsLateValue` in module and component scope; Window handles lower through `window_wid`.
- **Builtins:** `fetch` (request options plus `Response.text`/`json`/`arrayBuffer`/`clone`, `ok`, `status`, headers), `console.*` (`log`/`warn`/`error`/`info`), timers, `Promise` constructor + `await`, `Error`, `morphState`/`morphEffect`/`morphShared`/`morphEvent`.
- **Builtin namespaces:** `Math` constants plus `min`/`max`, rounding, logs, exponentials, trig, `hypot`, `fround`, and `random`; `JSON.parse`/`stringify` (single-argument forms), `Number.isFinite`/`isNaN`/`isInteger`, `Object.keys`/`values`/`entries`, `Array.isArray`, `String`/`Number`/`Boolean` conversions, `parseInt`/`parseFloat`.
- **Strings:** broad method coverage including `toUpperCase`, `toLowerCase`, `trim*`, `indexOf`, `substring`, `slice`, `replace*`, `split`, `includes`, `startsWith`, `endsWith`, `pad*`, `repeat`, `match*`, `search`, `normalize`.
- **Arrays:** `push`/`pop`, `join`, `slice`, `concat`, `includes`, `indexOf`, `lastIndexOf`, `at`, `reverse`, `toString`, plus `map`, `filter`, `forEach`, `reduce`, `find`, `findIndex`, `some`, `every`, `sort`, `splice`, `shift`, `unshift`, `fill`, `flat`, `flatMap` (multi-argument `push` and spread arguments on supported methods included). Receivers that call these lower to `JsArray`; `std::vector` stays the fast path for loops and indexing. A JSX-producing `.map` is still only valid as a direct JSX child.
- **Objects:** `has`, `keys`, `get`, `values`, `entries`.
- **Operators with JS semantics:** `==`/`!=`, `===`/`!==`, relational, `in`, `**`/`**=`, `&&=`/`||=`/`??=`, truthiness, string concatenation, `ToNumber` in numeric contexts.
- **JSX:** elements, fragments, and list `.map` in direct JSX-child position.

`Response.json()` returns a `JsValue`; dot access and the `await` form are Part 3A/3B. `JSON.parse`/`stringify` extra arguments are Part 0. Unannotated class fields are Part 1B.

## Part map

Parts are ordered so that a later part never changes a representation owned by an earlier one. Part 0 is the gate: no feature part starts until every known contract violation is either lowered or rejected.

| Part | Scope | Depends on | Owns | Exit criterion |
|---|---|---|---|---|
| **0** | Contract integrity: silent-wrong and check-pass/build-fail cases, capability tables, fail fixtures | — | linter capability tables, conformance policy | Zero known ⚠️/🟥 cases; every rejection has a fail fixture |
| **1A** | Widening bindings: initialized `let`, unions, `any`, late values | 0 | `var_types`, `JsLateValue`, `Signal<T>` | `let x = 0; x = "hi"` builds and matches JS |
| **1B** | Class model: unannotated/static/private fields, parameter properties, class expressions, getter/setter backing fields | 1A | class emitter | Unannotated and private fields compile |
| **1C** | Destructuring assignment and loop bindings | 0 | assignment/pattern emitter | `[a,b] = ...`, `({c} = o)`, `for (const [k,v] of ...)` lower |
| **1D** | Per-iteration closures | 0 | loop emitter | `for (let i...)` closures capture per iteration |
| **2A** | String `switch` | 0 | statement emitter | `switch` on strings matches JS `===` semantics |
| **2B** | `new.target` | 0 | expression emitter | No undeclared `JsNull`; correct constructor identity |
| **2C** | `for...in` over arrays | 0 | statement emitter | Indices, not values; own enumerable keys only |
| **2D** | Hoisting and TDZ | 1A | declaration analysis | `var` is `undefined`; `let`/`const` use-before-declaration errors |
| **2E** | Imported module side effects | 0 + module-bindings design | module builder | Top-level statements run once in import order |
| **3A** | JSON return typing and dot access | 0 | `JsValue` member lowering | `JSON.parse(...).name` and `response.json().name` work |
| **3B** | `await response.json()` | 3A | Response emitter | Await form compiles and returns parsed data |
| **3C** | `.then`/`.catch` decision | 0 | linter/docs | Either a limited chain works or check rejects it and docs stop showing it |
| **3D** | Timer extra arguments | 0 | timer emitter | `setTimeout(fn, 0, ...args)` forwards arguments |
| **3E** | Promise statics | 0 | Promise runtime | `resolve`/`all`/`allSettled`/`race`/`any` build and match JS result order |
| **3F** | `fetch` request/response docs | 0 | docs/tests | `fetch(url, { method, headers, body })` documented and fixture-covered |
| **4A** | `Date` subset | 0 | date runtime | Construction, `now`, `parse`, `getTime`, `toISOString`, getters |
| **4B** | `RegExp` literals + `.test`/`.exec` | 0 | regex runtime | Validation and search work; unsupported features documented |
| **4C** | `Map`/`Set` | 0 | collection runtime | JS key semantics and insertion order |
| **4D** | `Object`/`Array` statics | 0 | collection runtime | `Object.assign`/`fromEntries`/`hasOwn`, `Array.from`/`of` |
| **4E** | String/Number/URL/object methods | 0 | runtime helpers | The listed helpers work on native and `JsValue` receivers |
| **5A** | Rest parameters and general spread | 0 | call/expression emitter | `f(...args)` and `new C(...args)` lower correctly |
| **5B** | TypeScript enums | 0 | enum emitter | Numeric enums first; string enums documented |
| **5C** | `typeof`/`instanceof` | 0 | operator emitter | Runtime type tags and class checks match JS |
| **5D** | Namespace imports | module-bindings design | module graph | `import * as ns` resolves and compiles |
| **5E** | Function values in object literals | 0 | closure storage | `{ f: () => 1 }` stores and calls a closure |
| **5F** | Tagged templates and `arguments` | 0 | expression emitter | Both lower or stay explicitly rejected |
| **5G** | Lower-priority operators and build-time constants | 0 | operator/docs | `delete`/`void`/`>>>`; `import.meta.env` decision |
| **6A** | Persistent storage | 0 | platform API | File-backed storage; `localStorage` alias only if semantics match |
| **6B** | Clipboard | 0 | platform API | Read/write text through a documented API |
| **6C** | `requestAnimationFrame` | 0 | platform API | Next-frame coroutine |
| **6D** | Native dialogs | 0 | platform API | Open/save/message dialogs |
| **6E** | Global helpers | 0 | runtime helpers | `isNaN`, `isFinite`, URI helpers, `structuredClone`, `queueMicrotask`, `atob`/`btoa`, random UUID |
| **6F** | JSX spread props | 0 + prop analysis | component prop analysis | `<Card {...props} />` passes required-prop analysis |
| **6G** | Component children | 0 | JSX contract | Either supported or kept as an explicit v1 rejection with docs |
| **7** | Docs and claims | each part | docs + registry | User docs match behavior; registry stays in sync |

Parts 0–4 plus 6A–6D are the current app-logic path and block the three reference apps. Part 5 and the remaining Part 6 items are compatibility and platform breadth; they do not block the reference apps unless a specific app needs them. Part 7 is continuous.

## Part 0 — Contract integrity

**This is the gate.** Every item below is a known violation of the contract. Part 0 either lowers it correctly or rejects it with a precise `morph check` error. Each later part removes its own rejection and adds a pass fixture; no part is allowed to leave a check-pass/build-fail gap in between.

The statuses below were probed on 2026-11-04 with `morph check`, `morph --to cpp`, and `g++ -std=c++23 -fsyntax-only`. Re-probe before changing a row.

### 0.1 Silent-wrong behavior

| ID | Construct | Today | Target |
|---|---|---|---|
| 0.1.1 | `for (const i in [10, 20])` | 🟥 Iterates values (`10`, `20`) instead of indices (`"0"`, `"1"`). | Lower to index iteration over own enumerable keys, or reject array `for...in`. |
| 0.1.2 | `console.log(x); var x = 1` | 🟥 Prints `1`; JS prints `undefined`. | Hoist `var` with `undefined` initialization. |
| 0.1.3 | `console.log(x); let x = 1` | 🟥 Prints `1`; JS throws a TDZ `ReferenceError`. | Emit a TDZ check/error, or reject use-before-declaration. |
| 0.1.4 | `JSON.stringify(v, replacer, 2)` | 🟥 Extra arguments are silently dropped. | Implement `replacer`/`space`, or reject with a hint. |
| 0.1.5 | `JSON.parse(text, reviver)` | 🟥 `reviver` is silently dropped. | Implement `reviver`, or reject with a hint. |
| 0.1.6 | Top-level statements in an imported `.ts` | 🟥 Dropped; module initialization never runs. | Execute once in import order (Part 2E). |
| 0.1.7 | Codegen placeholders (`/* destructuring not supported */`, `/* spread */`, …) | 🟥 Some reach generated C++. | Every placeholder must become real lowering or a check error in the same change. |

### 0.2 Check-pass/build-fail behavior

| ID | Construct | Generated failure | Owning part |
|---|---|---|---|
| 0.2.1 | `let x = 0; x = "hi"`; `let x: number \| string`; `let y: any` | Type is fixed from the initializer (`int64_t x = 0`); later assignment emits a string. | 1A |
| 0.2.2 | `class A { x = 0 }`; `static count = 0`; `private _x = 0`; getter/setter backing fields | Emits `auto` data members — invalid C++. | 1B |
| 0.2.3 | `switch (someString)` | C++ `switch` on a non-integer. | 2A |
| 0.2.4 | `for (let i = 0; ...) fns.push(() => i)` | `i is not captured`; no per-iteration binding. | 1D |
| 0.2.5 | `[a, b] = [b, a]`; `({ c } = o)` | Destructuring-assignment placeholder / invalid C++. | 1C |
| 0.2.6 | `const C = class { ... }` | Invalid C++; check passes even though the overview lists it as unsupported. | 1B |
| 0.2.7 | `constructor(public x: number)` | Parameter property is never declared as a field. | 1B |
| 0.2.8 | `#private` fields | `auto` placeholder. | 1B |
| 0.2.9 | `fetch(...).then(...)`; `r.text().then(...)` | `Result<JsString>` / `std::string` has no `.then`. | 3C |
| 0.2.10 | `setTimeout(fn, 0, 1, 2)` | Callback is emitted as `std::function<void()>`; extra arguments are dropped. | 3D |
| 0.2.11 | `new.target` | Emits undeclared `JsNull{}`. | 2B |
| 0.2.12 | `for (const [k, v] of Object.entries(o))` | `for (auto /* array destructuring */ : ...)`; `k`/`v` undefined. | 1C |
| 0.2.13 | `JSON.parse(s).name`; `response.json().name` | `JsValue` has no `.name`; bracket access works. | 3A |
| 0.2.14 | `await response.json()` | `json()` is synchronous; translator emits `co_await` on a `JsValue`. | 3B |
| 0.2.15 | `<Card {...props} />` with required props | `.tsx` check passes; `morph build` fails `mx-component-required`. | 6F |
| 0.2.16 | `JSON.parse(...)` as the only JSON use | Can emit `morph::json_parse` without pulling in the runtime header. | 3A |

### 0.3 One capability table

The linter and translator currently duplicate supported-method/builtin lists. Part 0 extracts shared tables so a feature cannot be half-shipped:

- supported globals and namespace members;
- supported methods per receiver kind;
- operators with native lowering;
- syntax constructs with native lowering;
- rejected constructs and their `mx-*` codes.

Every later part adds rows to these tables; no part creates a second list.

### 0.4 Exit criteria

- Zero known ⚠️ and 🟥 cases.
- Every rejected construct has a precise `mx-*` error and a conformance fail fixture.
- Every accepted construct has a conformance pass fixture.
- `cargo test --workspace`, `check-js-types.sh`, `check-conformance.sh`, and `run-selftests.sh` are green.
- If the repo-wide clippy baseline is already red, record the baseline and require no new diagnostics.

## Part 1 — Types and bindings

### 1A — Widening bindings

**Today:** late-assigned values (`let x; x = 5; x = "hi"`) work through `JsLateValue`, but initialized bindings never widen:

```ts
let x = 0;
x = "hello";        // emits int64_t x = 0; x = "hello";
```

`number | string` and `any` annotations are ignored the same way.

**Target:** a binding whose later assignments or annotation widen its type uses `JsLateValue`; explicit annotations keep their native type until a widening assignment requires the general representation. Numeric/string/boolean/object/array/Window values keep working, and arithmetic lowers through `morph::js_late_*`.

**Scope:** module scope, function scope, component bodies, `let`/`var`, unions, `any`, nullish initialization, compound/logical assignments, and `++`/`--`.

**Depends on:** Part 0. Reuses the late-value machinery shipped in `7b9774e`.

**Owns:** `var_types`, `JsLateValue`, `Signal<T>`.

**Exit:** `let x = 0; x = "hi"` and `let x: number | string` build and match Node output; component-body signals seed correctly.

### 1B — Class model

**Today:** annotated fields work; unannotated fields, static fields, private fields, parameter properties, class expressions, and getter/setter backing fields do not.

**Target:**

- infer field types from the initializer and constructor assignments;
- widen fields to `JsLateValue` when later assignments require it;
- turn constructor parameter properties into real fields;
- lower `#private` to a private member with the same access rules;
- lower class expressions assigned to `const`/`let` into a named local class;
- keep getters/setters working when the backing field is unannotated.

**Depends on:** 1A.

**Owns:** class emitter.

**Exit:** `class A { x = 0 }`, `static count = 0`, `#x`, `constructor(public x: number)`, `const C = class { ... }`, and getter/setter classes compile and run.

### 1C — Destructuring assignment and loop bindings

**Today:** destructuring declarations work; assignment patterns and `for` binding patterns do not.

**Target:**

- `[a, b] = [b, a]`;
- `({ c } = o)`;
- defaults, rest elements, computed keys, and nested patterns in assignment position;
- `for (const [k, v] of Object.entries(o))` and other loop binding patterns;
- single evaluation of the right-hand side and computed keys.

**Depends on:** Part 0.

**Owns:** assignment/pattern emitter.

**Exit:** the patterns above build and match JS behavior, including defaults and rest.

### 1D — Per-iteration closures

**Today:** `for (let i = 0; ...) fns.push(() => i)` emits `i is not captured`.

**Target:** each loop iteration gets its own binding, so closures created in the body capture that iteration's value. This applies to `for`, `for...of`, and `for...in` where the binding is `let`/`const`.

**Depends on:** Part 0.

**Owns:** loop emitter.

**Exit:** a loop that pushes closures prints `0, 1, 2`, not the final value three times.

## Part 2 — Semantics and control flow

### 2A — String `switch`

**Today:** `switch` on a string passes check and emits a C++ `switch` on a non-integer.

**Target:** lower to an if/else chain or a tagged `JsValue` switch with JavaScript `===` semantics, including fallthrough and `default`.

**Depends on:** Part 0.

**Exit:** string, number, and mixed cases match Node; no fallthrough regressions.

### 2B — `new.target`

**Today:** passes check and emits undeclared `JsNull{}`.

**Target:** either implement constructor identity (for `new.target` checks) or reject with a precise error. Do not emit an undeclared placeholder.

**Depends on:** Part 0.

**Exit:** `new.target` inside a constructor and inside a called function behave like JS, or check rejects it.

### 2C — `for...in` over arrays

**Today:** silently iterates values instead of indices.

**Target:** iterate own enumerable string keys in JavaScript order, or reject array `for...in` until the correct lowering exists.

**Depends on:** Part 0 (silent-wrong fix).

**Exit:** `for (const i in [10, 20])` yields `"0"`, `"1"`; object `for...in` remains correct.

### 2D — Hoisting and TDZ

**Today:** use-before-declaration reads the initializer for both `var` and `let`.

**Target:** `var` is hoisted and initialized to `undefined`; `let`/`const` use-before-declaration produces a TDZ error (or a check error where runtime TDZ is not representable).

**Depends on:** 1A (binding model).

**Exit:** `console.log(x); var x = 1` prints `undefined`; `console.log(x); let x = 1` fails like JS or is rejected before codegen.

### 2E — Imported module side effects

**Today:** bare top-level statements in imported `.ts` modules are dropped.

**Target:** execute top-level statements once, in import order, with correct cycle behavior. Keep initialization in functions as the fallback only if the module system cannot represent it.

**Depends on:** Part 0 + the module-bindings design.

**Exit:** a module that registers state, logs, or initializes a table at top level runs exactly once when imported.

## Part 3 — Async, fetch, and JSON

### 3A — JSON return typing and dot access

**Today:** `JSON.parse(...)` and `response.json()` return `JsValue`. Bracket access works; dot access emits `obj.name` and fails because `JsValue` has no such member. A lone `JSON.parse` can also miss the runtime include.

**Target:** lower dot access on `JsValue` to the same lookup as bracket access, or reject it with a hint to use `["key"]` until typed JSON exists. Always pull the JSON runtime header when `JSON.parse` is used.

**Depends on:** Part 0.

**Owns:** `JsValue` member lowering.

**Exit:** `JSON.parse(text).name` and `response.json().name` build and read the field; `["name"]` remains equivalent.

### 3B — `await response.json()`

**Today:** `json()` is synchronous, but the translator emits `co_await` for the awaited form.

**Target:** either make `json()` awaitable or lower `await response.json()` to the synchronous call. The browser-shaped `await` must not produce invalid C++.

**Depends on:** 3A.

**Exit:** `const data = await response.json()` compiles and matches the synchronous form.

### 3C — `.then`/`.catch` decision

**Today:** `.then`/`.catch` pass check and fail in generated C++. The deployment guide now uses `async`/`await`, but the language still accepts `.then` and fails in C++.

**Target:** make an explicit decision:

- implement a small compatibility chain for `fetch`/`Response` results, or
- reject `.then`/`.catch` with a clear error and replace the docs example with `async`/`await`.

A general Promise chain runtime is not required. The contract only requires that the chosen behavior is honest.

**Depends on:** Part 0.

**Exit:** the deployment example builds, or check rejects it and the docs show the supported form.

### 3D — Timer extra arguments

**Today:** `setTimeout(fn, 0, 1, 2)` emits a `std::function<void()>` and drops the extra arguments.

**Target:** capture the trailing arguments in the callback for `setTimeout` and `setInterval`; keep `clearTimeout`/`clearInterval` behavior unchanged.

**Depends on:** Part 0.

**Exit:** `setTimeout((a, b) => console.log(a + b), 0, 1, 2)` prints `3`.

### 3E — Promise statics

**Today:** `Promise.resolve`/`all`/`allSettled`/`race`/`any` are rejected by `mx-js-member`.

**Target:** correct result ordering and rejection behavior; parallel execution is an optimization, not a requirement for the first version.

**Depends on:** Part 0.

**Owns:** Promise runtime.

**Exit:** a dashboard that awaits multiple requests through `Promise.all` builds and returns results in input order.

### 3F — `fetch` request/response docs

**Today:** the runtime supports `fetch(url, { method, headers, body })` and the full `Response` API, but the user docs only show the one-argument form.

**Target:** verify the codegen path, add examples for POST/headers/body and `response.headers.get(...)`, and add a conformance fixture.

**Depends on:** Part 0.

**Exit:** docs and fixtures cover GET, POST, headers, body, status, and JSON.

## Part 4 — Standard library

Each subpart is additive and owns its runtime header. Do not change `JsArray`/`JsObject` representation from a standard-library part; add overloads/helpers instead.

### 4A — `Date`

Construction, `Date.now`, `Date.parse`, `getTime`, `toISOString`, and calendar/time getters. Timezone-aware formatting is out of scope; document the supported subset.

### 4B — `RegExp`

Regex literals plus `.test` and `.exec`; document unsupported flags/features. Validation and text search are the target, not full regex parity.

### 4C — `Map` / `Set`

Insertion order, JS key semantics, `get`/`set`/`has`/`delete`/`size`, and iteration. `WeakMap`/`WeakSet` remain non-goals unless a concrete requirement appears.

### 4D — `Object` / `Array` statics

`Object.assign`, `Object.fromEntries`, `Object.hasOwn`, `Array.from`, and `Array.of`. Keep `Object.keys`/`values`/`entries` and `Array.isArray` working.

### 4E — String, Number, URL, and object methods

- `String.at`, `codePointAt`, `concat`, `String.fromCharCode`, `String.raw`.
- `Number.MAX_SAFE_INTEGER`, `Number.isSafeInteger`.
- `Object.hasOwnProperty` (or a documented `Object.hasOwn` replacement).
- `URL` / `URLSearchParams`.
- `Array.findLast`, `findLastIndex`, `toSorted`, `toReversed`, `toSpliced`, `with`, `reduceRight`, `copyWithin`.
- Remaining safe `Math`/`Number` members should be audited against the same table.

## Part 5 — Language surface

### 5A — Rest parameters and general spread

`function f(...args)`, `f(...args)`, and `new C(...args)`. Array spread and variadic array helpers already work; this part covers the general call/constructor cases. Rest destructuring stays supported.

### 5B — TypeScript enums

Numeric enums first; string enums are optional and must be documented. `const enum` behavior must be decided. The codegen already has an enum emitter; the linter gate is the blocker.

### 5C — `typeof` / `instanceof`

Runtime type tags for `typeof` strings and class identity for `instanceof`, including narrowing patterns such as `if (typeof x === "string")`. If a case cannot be represented, reject it rather than returning a wrong tag.

### 5D — Namespace imports

`import * as ns` resolves through the module graph and compiles to the defining namespace. Depends on the module-bindings design.

### 5E — Function values in object literals

`{ f: () => 1 }` requires a closure-storage decision. Implement it with an explicit function type or keep the construct rejected with a clear error; do not pass it through as a method name.

### 5F — Tagged templates and `arguments`

Tagged templates and the `arguments` object are currently rejected. Either lower them or keep them as documented limitations. They are lower priority than the app-logic items above.

### 5G — Lower-priority operators and build-time constants

`delete`, `void`, and `>>>`; `import.meta.env` build-time constants (or a documented alternative). The deployment guide now uses an explicit version constant instead of `import.meta.env.APP_VERSION`; a real build-time mechanism is still a requirement.

## Part 6 — Platform and JSX

### 6A — Persistent storage

File-backed key-value storage behind a Morph-native API. A `localStorage` alias is allowed only if its synchronous semantics and string-only values are honest.

### 6B — Clipboard

Read/write text through a documented API. Do not expose `navigator.clipboard` unless the native implementation matches its promises and permissions.

### 6C — `requestAnimationFrame`

Next-frame coroutine for animation loops. Prefer a Morph-native name if the browser callback signature cannot be matched exactly.

### 6D — Native dialogs

Open/save/message dialogs behind a documented API. `alert`/`confirm`/`prompt` aliases only where the semantics match.

### 6E — Global helpers

`isNaN`, `isFinite`, `encodeURIComponent`, `decodeURIComponent`, `structuredClone`, `queueMicrotask`, `atob`/`btoa`, `crypto.getRandomValues`/`randomUUID`, and the remaining safe `console` methods. Keep `console.table`/`debug`/`group` explicit if they are not implemented.

### 6F — JSX spread props

`<Card {...props} />` must participate in required-prop analysis. Today a `.tsx` check passes and `morph build` fails `mx-component-required` because the spread is not inspected.

### 6G — Component children

Component `children` are rejected in v1. Either implement the slot model or keep the rejection and make the docs/error message explicit. This is a product decision, not a codegen accident.

## Part 7 — Docs and claims

Docs ship with the feature, not after it:

- `docs/guides/deployment.md` — ✅ the `.then`/`import.meta.env` updater example now uses `async`/`await`, bracket JSON access, and an explicit version constant.
- `docs/javascript/async.md` — ✅ documents `fetch` request options, synchronous `response.json()` with bracket access, timer argument limits, and the unsupported Promise forms.
- `docs/javascript/overview.md` — ✅ “What’s Not Supported Yet” is accurate and links to this plan for known contract gaps.
- `docs/errors/*` — add or update the error page for every new rejection.
- `docs/docs.registry.json` and `docs/future/index.md` — keep titles, descriptions, keywords, and sidebar order in sync.
- Reference apps — add the CRUD/list, API dashboard, and settings fixtures as the acceptance gate.

## Dependency graph

```text
Part 0 (contract gate)
├─ 1A ──> 1B
│      └> 2D
├─ 1C
├─ 1D
├─ 2A, 2B, 2C
├─ 2E (after module-bindings design)
├─ 3A ──> 3B
├─ 3C, 3D, 3E, 3F
├─ 4A, 4B, 4C, 4D, 4E
├─ 5A, 5B, 5C, 5E, 5F, 5G
├─ 5D (after module-bindings design)
└─ 6A, 6B, 6C, 6D, 6E, 6G
   └─ 6F (after component prop analysis)
```

After Part 0, the independent branches can run in parallel. Only the arrows above impose ordering.

## Shared seams and ownership

A part may **add** to a seam, not change its representation. If a representation change is unavoidable, it lands in the owning part first with all consumers' fixtures green.

| Seam | Owner | Other parts may |
|---|---|---|
| `var_types`, `JsLateValue`, `Signal<T>` | 1A | read; add variants only through 1A |
| Class emitter | 1B | add class features only through 1B |
| Assignment/pattern emitter | 1C | add patterns only through 1C |
| Loop emitter | 1D | add loop forms only through 1D |
| Statement/expression emitter | 2A–2C | add new arms; no rewrites of existing arms |
| Declaration analysis | 2D | read; TDZ rules only through 2D |
| Module builder | 2E | coordinate with 5D; no parallel edits |
| `JsValue` member lowering | 3A | use it; no duplicate lowering |
| `morph::net::Response` | 3B/3F | add methods; no signature changes |
| Promise runtime | 3E | additive helpers only |
| Runtime containers (`JsArray`, `JsObject`, `Map`, `Set`) | 4C/4D | additive overloads only |
| Enum emitter | 5B | no parallel edits |
| Operator emitter | 5C/5G | add operators; no rewrites |
| Component prop analysis | 6F | read; no parallel edits |
| Linter capability tables | 0 | add rows; no second copy of the table |
| Docs registry | 7 | update with each part |

## Per-part acceptance

Every part must provide:

1. A runtime/helper unit test where a runtime type is involved.
2. A translator/codegen test for the emitted C++.
3. A conformance pass fixture under `tests/runtime/conformance/`.
4. A conformance fail fixture for every form the part still rejects.
5. Docs and registry updates.

Run from the repo root:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
./tests/runtime/check-js-types.sh
./tests/runtime/check-conformance.sh
./tests/runtime/run-selftests.sh
```

A part is not done until the reference app it unblocks builds without a `.cpp` helper.

## Explicit non-goals

Do **not** chase browser parity:

- DOM (`document`, `window`, `location`, `history`, `navigator`), event constructors, `addEventListener`.
- Full `Intl`, locale/timezone formatting, TypedArrays, `Proxy`, `Reflect`, `WeakMap`, `WeakSet`, `BigInt`, `Symbol`.
- Generators/`yield`, `with`, labeled jumps, dynamic `import()`, decorators, `eval`, the `Function` constructor.
- A general `Promise.then` chain runtime — Morph uses `async`/`await` coroutines. A small `fetch`-specific compatibility form is allowed only if Part 3C chooses it.
- Full npm/Node compatibility; that is a separate plan ([Node.js Support](nodejs.md)).
- Browser network APIs beyond HTTP `fetch` (`WebSocket`, `EventSource`, `XMLHttpRequest`) until a concrete app requires them.

Morph should expose native equivalents (`useWindow`, clipboard, storage) instead of pretending to be a browser.

## Current state

| Piece | State |
|---|---|
| Current baseline (above) | ✅ Shipped |
| Late-assigned JS values (`JsLateValue`, Window handles) | ✅ Shipped in `7b9774e` |
| General array methods, object spread, component-body destructuring | ✅ Shipped |
| `Response.json()` detection and full `Response` runtime | ✅ Shipped; dot access/await forms are Part 3A/3B |
| `morph check` diagnostics for rejected constructs | ✅ Shipped; grows with each part |
| App-logic contract and docs truth-up | ✅ This pass |
| Part 0 — contract integrity | 🚧 In progress |
| Parts 1–6 | 🚧 Planned |
| Part 7 — docs and claims | 📋 Continuous |

## How to use this page

1. Pick the lowest-numbered part whose dependencies are green.
2. Do not edit a seam owned by another part.
3. Make every new acceptance or rejection visible in `morph check` and in a conformance fixture.
4. Update this page and the registry when a part ships.
