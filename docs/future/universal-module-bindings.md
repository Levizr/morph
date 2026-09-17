# Universal Module Bindings — Import / Export / Native Calls

**Status:** `development` · **Priority:** high · **Shipped parts:** none (design record; implementation follows this page)

> Companion to [State, Events & Native C++ Interop](state-events-native-interop.md), which established module-path identity for `morphShared`/`morphEvent`. This page generalizes the same namespace trick to **every binding kind** (functions, vars, classes, components, shared, events) from **every module kind** (`.mx`/`.ts`/`.tsx`), plus re-exports and the C++ calling convention.

---

## 1. The Rule

Every binding lives in its **defining file's** module namespace. Imports and C++ both resolve to the definition site. No file needs its own isolation (no manual prefixing to avoid collisions) — the namespace does it:

```tsx
// network.ts
export function fetchUserData() { /* ... */ }

// utility.ts
import { fetchUserData } from './network.ts'
export function helper() { return fetchUserData() }

// Navbar.mx
import { loadData } from './utility.ts'
export default function App() {
  function refresh() { loadData() }   // rewritten: morph_mods::utility::loadData()
}
```

```cpp
// native.cpp — always the DEFINING namespace:
morph_mods::utility::loadData();    // ✅ defined in utility.ts
morph_mods::navbar::loadData();     // ❌ never exists (Navbar only imports it)
```

If `utility.ts` uses `fetchUserData()` from `network.ts` while `Navbar.mx` defines its own `fetchUserData()`, both coexist — `morph_mods::network::fetchUserData` vs `morph_mods::navbar::fetchUserData`. No overwriting, no duplication errors, no per-file isolation discipline.

---

## 2. Identity: Module Path + Name, for Everything

```
identity = canonical_module_path + "::" + binding_name
```

| Binding | Example identity |
|---|---|
| Function | `src/utility.ts::loadData` |
| Module var | `src/store.ts::token` |
| Class | `src/models.ts::User` |
| Component | `src/ui/Counter.mx::Counter` (existing) |
| Shared | `src/cart.ts::cart` (existing) |
| Event | `src/cart.ts::cartChanged` (existing) |

Namespaces reuse the one scheme from the state/events record: entry-relative segments, lowercased, `[a-z0-9_]`, `morph_mods::<…>`, `mx-naming` hard gates (now enforced for `.ts`/`.tsx` too). The old `<stem>_logic` fragment wrapper is retired by this scheme (it was stem-only — `a/utils.ts` and `b/utils.ts` collided).

---

## 3. Export Inventory (per module)

The parser records every exportable binding (morpher already emits functions, vars, and classes via `emit_class`, so all three are inventoryable):

- **Named**: `export function f`, `export const x`, `export class C`, `export { a, b }`.
- **Default**: `export default function f` / `export default class C` — importable as `import f from './m'` or `import { default as f } from './m'`; the binding keeps its declared name in the namespace.
- **Types/interfaces**: erased, no C++ emission. `import type { T }` is always fine; a value-position `import { T }` where `T` is type-only is a hard error with an `import type` hint.

---

## 4. Import Resolution

`import { x } from './y'` (or default import) resolves `x` against `y`'s full inventory — component, shared, event, function, var, class:

- Found → every use rewrites to `morph_mods::<ns_y>::x` (same rename machinery shared state already uses).
- Specifier matches nothing in `y` → **hard error** (no silent fall-through).
- Same local name imported from two modules → **hard error** (existing ambiguous-import rule, now universal).
- No re-declaration in the importing namespace: the name exists only at its definition site.

---

## 5. Re-exports

Re-exports are allowed (`export { loadData } from './utility.ts'`, `export * from './utility.ts'`), and C++ can call re-exported things — through a zero-cost alias in the re-exporting namespace:

```cpp
// Generated for Navbar.mx re-exporting loadData from utility.ts
namespace morph_mods { namespace navbar {
using morph_mods::utility::loadData;
}}
```

So `morph_mods::navbar::loadData()` works **iff** `Navbar.mx` re-exports it; a plain (non-re-exporting) import creates no alias. Rules:

- Re-export of an unknown name → hard error.
- Re-export name colliding with a local definition → hard error (explicit beats implicit; rename one).
- Star re-exports that collide (`export *` from two modules exporting the same name) → hard error.
- `using`-aliases are declarations only — one definition still lives at the defining site, so no linker duplication.

---

## 6. C++ Calling Convention

`morph_api.h` declares **all** module bindings (functions, vars, classes — `.mx` and `.ts` alike) with mapping comments; definitions live once at their namespace. Native code includes one header and calls the defining namespace:

```cpp
#include "morph_api.h"

void refreshAll() {
    morph_mods::utility::loadData();          // defined in utility.ts
    morph_mods::store::setToken("abc");       // defined in store.ts
    morph_mods::models::User u("hero");       // defined in models.ts
}
```

Decision tree addition (extends the `native-cpp.md` table): *call a TS/TSX function* → `morph_mods::<defining_module>::name()`; *find the defining module* → the `// name (path/file:line)` comment in `morph_api.h`.

---

## 7. `.ts` Strategy (DX + performance)

- **One namespace scheme everywhere**: fragments wrap in `morph_mods::<ns>` (not `<stem>_logic`).
- **Keep separate fragment TUs** (one `.ts.cpp` per source, as today) instead of inlining everything into `app.cpp`: parallel compilation, incremental rebuilds (unchanged files skip), and the existing `-ffunction-sections` + `--gc-sections` already dead-strip unused helpers. DX is unaffected — names are predictable and the header is the only surface that matters.
- `.ts` → `.mx` calls work today (same binary); `.mx` → `.ts` calls route through the new import resolution (previously bare names that only linked by luck).

---

## 8. What Changes per Layer

| Layer | Change |
|---|---|
| `morph-parser` (`js_walker`) | Inventory classes + exported module vars + default-export mapping; `mx-naming` enforced for `.ts` |
| `morph-parser` (`linter`) | Unknown/ambiguous specifier lints cover all binding kinds; type-only guidance |
| `morph-ir` (`builder`) | One binding registry (`{key, kind, ns, name, sig/init, module}`); specifier resolution against full target inventory; re-export alias entries |
| `morph-codegen` (`cpp`) | Per-namespace definitions for functions/vars/classes; `morph_api.h` declares everything with mapping comments; retire `<stem>_logic` wrapper |
| `morph-codegen` (`logic_emitter`) | Dev TU + `_morph_state.h`: same qualification, no bare re-declarations |
| `morpher` | No changes (already emits all three binding kinds) |

---

## 9. Status

| Item | Status | Notes |
|---|---|---|
| Design record (this page) | ✅ Shipped | Decided with reviewer 2026-09-17 |
| Parser inventory (classes, exported vars, default mapping) | ✅ Shipped | `emit_class` existed; inventory was the gap |
| Builder registry + import resolution + re-export aliases | ✅ Shipped | Generalizes `register_binding`/`register_event`; unknown/ambiguous/cycle = hard errors |
| Codegen namespaced defs + full `morph_api.h` | ✅ Shipped | Func decls, var externs, moved classes, `using`-aliases; single-pass substitution (no double-qualify) |
| Fragments of graph-member `.ts` skipped | ✅ Shipped | Builder owns graph modules; unimported files keep fragments |
| `native-cpp.md` binding-call examples | ✅ Shipped | Decision tree + defining-namespace rules |
| Unified `morph_mods::<ns>` fragments (retire `<stem>_logic`) | 📋 Planned | Standalone (unimported) files still use it; unify when touched |
| `import type` erasure guidance | 📋 Planned | Linter hint |
| Top-level side-effect statements in imported `.ts` | 📋 Known limitation | Builder drops bare statements; keep init code in functions |

---

## 10. FAQ

### Why not re-declare imported names in the importing namespace?

Because then two importers of `loadData` would each define it (linker duplication), and C++ couldn't tell the definition from the alias. One definition + `using`-aliases only for explicit re-exports keeps a single source of truth — the exact failure (`navbar::loadData` must not exist) from the motivating example.

### Why keep `.ts` fragments as separate TUs?

Compile-time parallelism and incrementality. The old objection (unpredictable `<stem>_logic` names) disappears with the unified namespace; linkage of namespaced definitions across TUs is plain C++ with zero overhead.

### What about name shadowing inside the importing file?

A local `const loadData` next to `import { loadData }` is an ambiguous-import hard error — same rule as shared/event imports. Rename one.

---

## 11. Decision Log

| Date | Decision | Rationale |
|---|---|---|
| 2026-09-17 | Universal module-path identity for all binding kinds | Same trick that solved shared/events; kills the `fetchUserData` collision class |
| 2026-09-17 | Re-exports allowed; C++-callable via `using`-alias | Explicit re-export = explicit alias; plain imports create nothing |
| 2026-09-17 | Default exports bind local name → qualified defining name | Matches ES semantics with zero new syntax |
| 2026-09-17 | Keep separate `.ts` fragment TUs | Parallel + incremental builds; DX unchanged via unified namespaces |
| 2026-09-17 | Retire `<stem>_logic` for `morph_mods::<ns>` | Stem-only namespaces collide across directories |
| 2026-09-17 | `import type` always fine; value-position type imports error | Types are erased; the hint teaches the escape hatch |

---

*Related: [State, Events & Native C++ Interop](state-events-native-interop.md) · [C++ / JSX Interop Guide](../guides/native-cpp.md) · [Dev Internals: State & Events](../dev/state-events-internals.md)*
