# Morph State, Events & Native C++ Interop — Complete Design Record

**Status:** `development` · **Priority:** high · **Shipped parts:** namespaced `morphShared` / `morphEvent`, strict TS-only gates, per-module C++ namespaces, `morph_gen.h` generation

> This page documents the complete architecture for Morph's reactive system (state, events, effects) and native C++ interop, as finalized through design review. Syntax and internal APIs shown here are implemented — behavior changes would be a breaking release.

---

## 1. Identity Model: Module Path + Binding Name

No string keys anywhere. Every shared binding and event channel is identified by:

```
identity = canonical_module_path + "::" + binding_name
```

- `morphShared` export: `src/auth/login.ts::isLoggedIn`
- `morphEvent` export: `src/events.ts::toastEvent`

Importing the same binding from the same module = same signal/channel. Same binding name in a different file = different identity. Importing one local name from two modules = **hard build error** (ambiguous import).

---

## 2. Naming Rules (Hard Gates)

### File / directory names
- **Normalization:** all segments lowercased at compile time (`Store.mx` → `store`, `Auth/` → `auth`).
- **Alphabet:** `[a-z0-9_]` only per segment after lowercasing. Extensions `.mx` `.ts` `.tsx` ignored for stem.
- **Collision:** two paths that normalize to the same segments → **hard error** (even on case-insensitive FS, even if original casing differed).
- **No renames needed:** existing CamelCase fixtures (`Store.mx`, `ShopStore.mx`, etc.) automatically comply once lowercased.

### `mid` (Morph Instance Descriptor)
- **Purpose:** make a specific component instance addressable from native C++.
- **Alphabet:** letters only (`a-zA-Z`), any case accepted, lowercased at compile time (`"Hero"` ≡ `"hero"`). **Digits banned** — identifiers must stay readable and unambiguous.
- **Duplicates:** same `mid` (post-lowercase) on same component type → hard error. Different component types may reuse `mid` (namespaced per component).
- **Literals only:** `mid="hero"` valid, `mid={var}` → hard error. The string must resolve to a build-time integer constant.
- **Reserved:** `mid` is reserved (like `key` for lists), never passed as a prop.
- **Lists:** `mid` on list template items is a **hard error** until per-item state lands (current limitation documented).

---

## 3. C++ Namespaces (Generated, Predictable)

Root: `morph_mods` (kept to minimize diff). Then path segments, then stem. **No hash leaf** — hard naming rules guarantee 1:1 mapping.

```cpp
// Generated for src/components/shop/ShopStore.mx exporting `cart`
namespace morph_mods {
namespace components {
namespace shop {
namespace shopstore {      // stem: "shopstore" (lowercased from ShopStore.mx)
    static morph::Signal<int>& shared_cart() { static morph::Signal<int> s(0); return s; }
    static morph::Channel& evt_cartChanged() { static morph::Channel c; return c; }
}}}}
```

- **Shared state accessor:** `shared_<sanitized_getter>()`
- **Event channel accessor:** `evt_<sanitized_event_name>()`
- **No hash leaf:** fully human-computable from file path. Hard rules = no collisions.
- **Emission:** every signal/channel is a function-local static (thread-safe lazy init, ODR-safe by construction).

---

## 4. `morph_gen.h` — The Native Developer's Contract

Single generated header, included by user `native.cpp`. It is **the** discovery surface: a C++ developer writes native code against this header without ever reading `app.cpp`.

### Contents
- Shared state accessors + thin setter/notify wrappers:
  ```cpp
  namespace components::shop::shopstore {
      inline int get_cart() { return shared_cart().get(); }
      inline void set_cart(int v) { shared_cart().set(v); }
      inline void notify_cartChanged() { evt_cartChanged().emit(JsObject{}); }
  }
  ```
- Event channel accessors: `evt_<name>()`.
- `mid` constants (per component type):
  ```cpp
  namespace components::counter {
      constexpr uint32_t MID_HERO = 0;   // <Counter mid="hero"> (App.mx:16)
      constexpr uint32_t MID_FIVES = 1;  // <Counter mid="fives"> (App.mx:17)
      inline void set_count(uint32_t mid, int v) {
          if (mid < 2 && (s_alive & (1u << mid))) s_count[mid].set(v);
      }
  }
  ```
- **Mapping comments:** every entry carries `// <Counter mid="hero"> (App.mx:16)` so the header is self-documenting.
- **No templates beyond `Signal<T>`** (already in the runtime). No type erasure, no `morph::instance<T>`, no registry object, no `unordered_map`.

---

## 5. User `native.cpp` — The DX Target

### Shared state (read/write)
```cpp
#include "morph_gen.h"

void resetCart() {
    components::shop::shopstore::set_cart(0);           // thin wrapper → shared_cart().set(0)
    components::shop::shopstore::notify_cartChanged();  // thin wrapper → evt_cartChanged().emit({})
}
```

### Events (emit / subscribe)
```cpp
// Emit from anywhere (native thread safe):
components::shop::shopstore::evt_cartChanged().emit(JsObject{{"total", components::shop::shopstore::get_cart()}});

// Subscribe in generated rewire only (user doesn't write this):
components::shop::shopstore::evt_cartChanged().on([](const JsValue& p) { ... });
```

### Specific instance from native (opt-in `mid`)
```cpp
// User wrote: <Counter mid="hero" /> in TSX
void resetHeroCounter() {
    components::counter::set_count(components::counter::MID_HERO, 0);
}
```

### Pure compute (unchanged DX)
```cpp
int doubleIt(int x) { return x * 2; }   // zero plumbing, always
```

---

## 6. Decision Tree (from `native-cpp.md`)

| Need | Route | Code |
|---|---|---|
| Read shared state | `get_<binding>()` | `components::shop::shopstore::get_cart()` |
| Write shared state | `set_<binding>(v)` + optional `notify_<event>()` | `set_cart(0); notify_cartChanged();` |
| Emit event | `evt_<name>().emit(payload)` | `evt_cartChanged().emit({});` |
| Read another component's local | **Don't.** Emit event; let that component reset its own local. | `evt_resetEvent().emit({});` / `resetEvent.on(() => setCount(0))` |
| C++ produces value for local | Return it; TSX handler writes its own local. | `int compute();` / `<button onClick={() => setCount(compute())}>` |
| **Native initiates write to specific instance** | Tag `<Comp mid="tag" />`; use `MID_TAG` constant. | `set_count(MID_HERO, 0)` |
| List items | **Hard error** — per-item state not yet supported. | — |

---

## 7. Events: Static Channels, Zero String Lookup

- One `morph::Channel` static per event binding inside its module namespace (`evt_<name>()`).
- `frame.events` maps event name → C++ accessor expression at IR build time.
- Codegen lowers `.emit()` and `.on()` directly to the accessor — **strings vanish from all emit sites**.
- `morph::channel_registry()` and `clear_channels()` kept **only for dev-mode rewire** (dev server needs to re-register subscriptions on hot reload). Production builds can strip the registry entirely.

---

## 8. Local State & Instances

### Current (shipped)
- Per-instance locals are **compile-time statics**: `__st_inst0_count`, `__st_inst1_count`, etc.
- Instance count known at build time; lambda closures hardcode their instance's static.
- **Limitation:** list items share one slot per template (documented). `mid` on list items = hard error.

### Future (`mid` support)
- Build-time `mid` → `constexpr` integer constant + indexed accessor over existing statics + liveness bitmask.
- Unmount clears the bit → stale index → silent no-op (no crash, no ghost writes).
- No runtime registry, no string lookup, no templates beyond existing `Signal<T>`.

---

## 9. Current State (What Ships Today)

| Feature | Status | Notes |
|---|---|---|
| `morphShared` namespaced, `morphEvent` namespaced | ✅ Shipped | `morph_mods::<path>::shared_<name>()`, `evt_<name>()` |
| Strict TS-only sources (`.ts`/`.tsx`/`.mx`) | ✅ Shipped | `.js`/`.jsx` rejected at parse |
| `mid` parser + dedupe + codegen constants | 🔧 In progress | Parser collects, codegen emits constants + indexed accessors |
| Static event channels (no string registry) | 🔧 In progress | Channel statics in namespace, `frame.events` → accessors |
| `morph_gen.h` generation | 🔧 In progress | Shared/event accessors + `mid` constants + thin wrappers |
| Naming-rule linter (`mx-naming`) | 📋 Planned | `morph check` enforces lowercasing + collision |
| `morph_gen.h` in `native-cpp.md` | 📋 Planned | Rewrite with decision tree + examples |

---

## 10. Open / Deferred

| Item | Status | Reason |
|---|---|---|
| Per-item state in `.map` + `mid` on list items | Deferred | Requires dynamic per-item storage; current limitation documented |
| Dev-chosen integer `mid` (vs compiler ordinal) | Deferred | `mid` string is the primary form; integer ordinals for untagged |
| `morph_gen.h` devtools integration | Deferred | Header already the discovery surface; IDE indexing later |
| Registry deletion (prod builds) | Deferred | Keep dev registry for rewire; prod strip is a separate pass |

---

## 11. Related Pages

- [API Reference: morphState](../api/morphState.md) · [morphShared](../api/morphShared.md) · [morphEvent](../api/morphEvent.md) · [morphEffect](../api/morphEffect.md)
- [FAQ: choosing API](../api/faq/choosing.md) · [FAQ: morphShared](../api/faq/morphShared.md) · [FAQ: morphEvent](../api/faq/morphEvent.md)
- [Dev Internals: State & Events](state-events-internals.md) — compiler pipeline details
- [C++ / JSX Interop Guide](../guides/native-cpp.md) — user-facing native.cpp guide
- [Compiler & CLI](compiler.md) — Rust compiler shipped Sept 2026

---

## 12. Decision Log (for reviewers)

| Date | Decision | Rationale |
|---|---|---|
| 2026-09-16 | Lowercase normalization + collision = hard error | Predictable namespaces; works on case-insensitive FS; no renames needed |
| 2026-09-16 | `mid` = letters-only, literal-only, per-type unique | Build-time integer resolution; zero runtime strings; human-readable identity |
| 2026-09-16 | Drop hash leaf from namespaces | Hard naming rules = 1:1 mapping; hash adds DX tax |
| 2026-09-16 | Drop `global::` prefix | No `local::` sibling exists; redundant |
| 2026-09-16 | `mid` on lists = hard error | Per-item state not implemented; positional `::N` rots on reorder |
| 2026-09-16 | Indexed accessors + bitmask, no registry/templates | Same unmount safety, zero templates/maps/heap; plain C semantics |
| 2026-09-16 | Static `Channel` per event in namespace | Eliminates string-keyed registry lock + lookup on every emit |

---

*End of design record. This page is the source of truth for implementation and docs.*