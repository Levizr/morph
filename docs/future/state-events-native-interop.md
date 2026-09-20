# Morph State, Events & Native C++ Interop — Complete Design Record

**Status:** `development` · **Priority:** high · **Shipped parts:** namespaced `morphShared` / `morphEvent`, strict TS-only gates, per-module C++ namespaces, static event channels, `morph_api.h` generation

> **DX contract:** `app.cpp` may be as complex as the compiler needs — nobody writes it by hand. `native.cpp` (what the user writes) and `morph_api.h` (what the user includes) are sacred: short namespaces, thin wrappers, mapping comments, zero string plumbing. Every implementation choice below is judged by user-side DX first.

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

### `mid` (Morph ID — C++-side component-instance identity)
- **Purpose:** make a specific component instance addressable from native C++ (`MID_*` + indexed accessors). Frontend identity is `id`; the two are independent and coexist on one component (`<Comp id="card" mid="hero" />` — `id` for CSS/hooks, `mid` for C++).
- **Use-sites only:** `mid` appears only where a component is reused (`<Hero mid="something" />`) — never inside a component definition (declaring a `mid` prop is a hard error; `mid` is reserved like `key`, never passed as a prop), never on native elements (hard error), never on the entry component (it is never instantiated).
- **Alphabet:** letters only (`[a-zA-Z]`), any case accepted, lowercased at compile time (`"Hero"` ≡ `"hero"`). **Digits and symbols banned** — identifiers stay readable and unambiguous.
- **Duplicates:** same `mid` (canonical form) on same component type → hard error. Different component types may reuse `mid` (namespaced per component).
- **Literals only:** `mid="hero"` valid, `mid={var}` → hard error. Identity must resolve at build time.
- **Lists:** `mid` inside `.map()` item templates is a **hard error** until per-item state lands (current limitation documented).

---

## 3. C++ Namespaces (Generated, Predictable)

Root: `app` (kept to minimize diff). Then path segments, then stem. **No hash leaf** — hard naming rules guarantee 1:1 mapping.

```cpp
// Generated for src/components/shop/ShopStore.mx exporting `cart`
namespace app {
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

## 4. `morph_api.h` — The Native Developer's Contract

Single generated header per project (written to the build output dir next to `app.cpp`), included by user `native.cpp`. It is **the** discovery surface: a C++ developer writes native code against this header without ever reading `app.cpp`.

Name note: the runtime ships a base header also called `morph_api.h` (JS value types, `Signal<T>`, node/event/task primitives). The generated per-project header **shadows it by include order** (output dir is searched first) and re-includes the same runtime subpaths explicitly, so it is a strict superset: everything the base header provides plus this project's accessors, wrappers, and `MID_*` constants. User code keeps writing `#include "morph_api.h"` — one include, full API.

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
      inline void setCount(uint32_t mid, int v) {
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
#include "morph_api.h"

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
    components::counter::setCount(components::counter::MID_HERO, 0);
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
| Read shared state | `<binding>()` (JSX name) | `app::cartstore::cart()` |
| Write shared state | `<setter>(v)` + optional `notify_<event>()` | `setCart(0); notify_cartChanged();` |
| Emit event | `evt_<name>().emit(payload)` | `evt_cartChanged().emit({});` |
| Read another component's local | **Don't.** Emit event; let that component reset its own local. | `evt_resetEvent().emit({});` / `resetEvent.on(() => setCount(0))` |
| C++ produces value for local | Return it; TSX handler writes its own local. | `int compute();` / `<button onClick={() => setCount(compute())}>` |
| **Native initiates write to specific instance** | Tag `<Comp mid="tag" />`; use `MID_TAG` constant. | `setCount(MID_HERO, 0)` |
| List items | **Hard error** — per-item state not yet supported. | — |

---

## 7. Events: Static Channels, Zero String Lookup

### Before (string-based registry)
The compiler used to emit a string channel ID (`evt:<module path>::<name>`) and a global `std::map<std::string, Channel>` registry at every emit site:

```cpp
// Old generated output (had runtime overhead)
morph::channel("evt:/home/user/project/src/components/ShopStore.mx::clearCart").on([](const JsValue& __ch_0) { ... });
morph::channel("evt:/home/user/project/src/components/ShopStore.mx::clearCart").emit(JsObject{{"cart", ...}});
```

Every `.emit()` and `.on()` paid: mutex lock + `std::map` string lookup + listener vector copy. The absolute path literals also bloated the binary.

### Shipped: Namespace-Static Channels (Zero String Lookup)
The generated `morph_api.h` defines one `Channel` per event binding as an `inline` accessor inside its module namespace (definitions live in the header so user code included earlier in `app.cpp` sees them; `inline` keeps them TU-safe):

```cpp
// Generated morph_api.h for src/components/ShopStore.mx exporting `clearCart`
namespace app {
namespace store_shopstore_1a2b3c4d {
inline morph::Channel& evt_clearCart() { static morph::Channel c; return c; }
// User DX wrappers in the same header:
inline void emit_clearCart(const JsValue& p) { evt_clearCart().emit(p); }
inline void notify_clearCart() { evt_clearCart().emit(JsObject{}); }
}}
```

The build TU lowers every `morph::channel("<id>")` at emit/subscribe sites to the accessor — **strings vanish from all emit sites**:

```cpp
// Shipped generated output (zero string lookup)
app::store_shopstore_1a2b3c4d::evt_clearCart().emit(JsObject{{"cart", app::store_shopstore_1a2b3c4d::shared_cart().get()}});
app::store_shopstore_1a2b3c4d::evt_clearCart().on([](const JsValue& __ch_0) { __st_inst1_qty.set(0); });
```

- **Cost:** only the listener-list lock inside `emit` (inherent to pub/sub). No registry mutex, no string map, no string compares.
- `morph::channel_registry()` and `clear_channels()` are kept **only for the dev TU** (`morph dev` re-registers subscriptions on hot reload via `morph_logic_rewire`). Production builds never touch the registry.

---

## 8. Local State & Instances

### Current (shipped)
- Per-instance locals are **compile-time statics**: `__st_inst0_count`, `__st_inst1_count`, etc.
- Instance count known at build time; lambda closures hardcode their instance's static.
- **Limitation:** list items share one slot per template (documented). `mid` on list items = hard error.

### Shipped (`mid` support)
- Build-time `mid` → `constexpr` integer constant (`MID_HERO`) + `set_<state>(mid, v)` / `get_<state>(mid)` with switch dispatch over the existing `__st_` statics. Index covers tagged instances only, so untagged instances never shift `MID_*` values.
- **No liveness bitmask — deliberately.** Instance statics are build-time constants with no dynamic lifetime (conditional branches detach nodes, never free signals), so there is nothing to track: a write to a detached instance updates its own static harmlessly and can never touch another instance. Unknown indexes hit `default:` → silent no-op (no crash, no ghost writes).
- No runtime registry, no string lookup, no templates beyond existing `Signal<T>`.
- Every build embeds `binary --morph-self-test`: headless assertions over shared roundtrips, event delivery, and `mid` indexed writes (runs before GLFW init; see `tests/runtime/run-selftests.sh`).

---

## 9. Current State (What Ships Today)

| Feature | Status | Notes |
|---|---|---|
| `morphShared` namespaced, `morphEvent` namespaced | ✅ Shipped | `app::<path>::shared_<name>()`, `evt_<name>()` |
| Strict TS-only sources (`.ts`/`.tsx`/`.mx`) | ✅ Shipped | `.js`/`.jsx` rejected at parse |
| `mid` parser + dedupe + codegen constants | ✅ Shipped | Builder validates/records, codegen emits `MID_*` + switch-dispatch accessors + header decls |
| Static event channels (no string registry) | ✅ Shipped (build) | Channel statics in namespace; build TU lowers `morph::channel("<id>")` → accessor; dev TU keeps registry for rewire |
| `morph_api.h` generation | ✅ Shipped | Shared/event accessors + thin wrappers + mapping comments + `MID_*` |
| Naming-rule linter (`mx-naming`) | ✅ Shipped | `morph check` enforces lowercasing + collision; namespaces are hash-free and human-computable |
| `morph_api.h` in `native-cpp.md` | ✅ Shipped | Rewritten with decision tree + shared/event/mid examples + self-test |
| Headless runtime self-tests | ✅ Shipped | `binary --morph-self-test` + `tests/runtime/run-selftests.sh` over both fixtures |

---

## 10. Open / Deferred

| Item | Status | Reason |
|---|---|---|
| Per-item state in `.map` + `mid` on list items | Deferred | Requires dynamic per-item storage; current limitation documented |
| Dev-chosen integer `mid` (vs compiler ordinal) | Deferred | `mid` string is the primary form; integer ordinals for untagged |
| `morph_api.h` devtools integration | Deferred | Header already the discovery surface; IDE indexing later |
| Registry deletion (prod builds) | Deferred | Keep dev registry for rewire; prod strip is a separate pass |

---

## 10. FAQ

### Why `mid`? Why not just let native code call `setState()` directly?

The old syntax you're imagining — a bare `setState()` callable from anywhere — only works if there's exactly one instance of that component in the entire app. But Morph components are reusable: you can render `<Counter />` three times on one screen, and each has its own independent `count`. A bare `setState()` has no way to know *which* instance it should target.

`mid` solves this by making instance identity **explicit and opt-in**:

```tsx
<Counter mid="hero" />        // opt in: this instance is addressable
<Counter />                    // opt out: purely local, no native access
```

```cpp
// native.cpp — only works for instances that opted in
components::counter::setCount(components::counter::MID_HERO, 0);
```

If you don't need native code to poke a specific instance, don't add `mid` — the component stays purely self-contained. This is the "pay for what you use" principle: no overhead, no registry, no lifetime bugs for components that don't need cross-instance access.

The alternative (a global registry you query by component name + instance index) introduces exactly the problems `mid` avoids: implicit lifetime coupling, silent writes to dead instances, and a hidden global that makes testing and reasoning about code harder.

### Why did we remove string-based identity (`morphShared("cart", 0)`) in favor of namespace-based identity?

The string-key API was a global mutable map with no owner:

```tsx
// Old (removed)
morphShared("cart", 0)    // string key = global namespace
```

**Problems with string keys:**

| Problem | String Keys | Namespace Identity |
|---|---|---|
| **Collision detection** | Runtime (first write wins silently) | **Compile-time hard error** |
| **Refactor safety** | Rename key → silent bug everywhere | Rename binding → compiler points at every use |
| **IDE support** | None (string is opaque) | **Full autocomplete, go-to-definition** |
| **Cross-team coordination** | Must agree on string keys out-of-band | File path *is* the contract; no coordination needed |
| **Native C++ mapping** | String → lookup at runtime | Direct static accessor, zero lookup |

With namespace identity, the module system does the work:

```tsx
// src/cart.ts
export const [count, setCount] = morphShared(0)
```

```tsx
// Any other file — import IS the registry
import { count, setCount } from './cart'
```

The compiler knows exactly which `count` you mean because the import path is the identity. Two files can both declare `count` — they're different stores because their module paths differ. Importing the same name from two modules = **hard error** forcing you to rename (`import { count as cartCount }`).

This is why the old API was removed entirely (`mx-api-removed` lint): it encouraged patterns that don't scale, and the replacement is strictly better in every dimension.

### Why `morphShared` at module scope instead of `useShared()` inside components like React hooks?

React's `useShared` (or `useContext`) couples shared state to the component tree — the provider must be an ancestor. Morph's `morphShared` at module scope is **decoupled from the tree**:

```tsx
// Theme store — no provider needed, no tree coupling
// src/themeStore.ts
export const [theme, setTheme] = morphShared<'light' | 'dark'>('light')
```

```tsx
// Any component, anywhere in the tree, just imports
import { theme, setTheme } from './themeStore'
```

**Benefits of module-scope:**

- **No provider nesting** — shared state doesn't force you to wrap your app in `<ThemeProvider><AuthProvider><CartProvider>…`
- **No re-render cascades** — reading `theme` subscribes *only that component*; writing `setTheme` notifies *only subscribers*. React context re-renders all consumers on any value change.
- **Works outside components** — native code, background threads, timers, event handlers can all read/write via the same `morph_api.h` accessors. No "must be inside a component" restriction.
- **Testable in isolation** — import the store in a unit test, call `setTheme('dark')`, assert behavior. No need to render a provider tree.

The trade-off: you must export the binding (`export const …`) and the linter enforces module scope (`mx-shared-scope`). This is intentional: it makes shared state **explicitly opt-in and discoverable**, not an implicit side effect of rendering.

### Why is `mid` literal-only (`mid="hero"`) and not dynamic (`mid={id}`)?

`mid` resolves to a **build-time integer constant**. The compiler generates:

If you need dynamic instance addressing (e.g., user-created widgets), use a `morphShared` map keyed by user-provided IDs instead — that's what the shared store is for. `mid` is for **statically known, developer-chosen** instances.

### Why are we replacing string-based event channels (`morph::channel("evt:...")`) with namespace-based static channels?

The current string-based system has real per-emit overhead that never goes away:

```cpp
// Current: every emit/subscribe pays this cost
morph::channel("evt:/home/user/project/src/components/ShopStore.mx::clearCart").on(...);
```

**Per-emit cost of strings:**
1. `channel_registry_mutex()` lock + unlock
2. `std::map<std::string, Channel>` lookup (tree walk + string compares)
3. `Channel::emit`: second mutex + heap vector copy of listeners

Compare `Signal::set`: **one** mutex + notify. Events pay roughly 2× the synchronization of state writes, plus a string-keyed map lookup — on *every* emission and every `.on()` registration.

The fix is exactly what you'd expect: **the channel ID only needs to exist at build time**. The compiler already knows every event statically (`event_bindings` per module), so it generates one `Channel` object per event as a namespace static — the same pattern shared signals already use:

```cpp
// Generated — one static per event, same pattern as shared signals
namespace app { namespace components { namespace shopstore {
static morph::Channel& evt_clearCart() { static morph::Channel c; return c; }
}}}
```

Then emit/subscribe become direct calls. Remaining cost: only the listener-list lock inside `emit`, which is inherent to pub/sub. And the absolute-path literals no longer bloat the binary at every emit site.

This is the same pattern we already use for shared signals — we're just applying it to events too.

### Why letters only in `mid`? Why no digits?

Two reasons:

1. **Readability at a distance:** `mid="hero"` reads like a name; `mid="item1"` reads like a database key. The namespace already encodes structure — `counter::MID_HERO` is self-explanatory.
2. **Sanitization ambiguity:** `item1` + `item12` → what's the lowered form? `item1` vs `item12` are distinct, but `item_1` vs `item1` collides in unpredictable ways. Banning digits makes the lowered form 1:1 with the source.

If you need multiple similar instances, use descriptive names: `heroPrimary`, `heroSecondary`, `fives`, `tens`, etc. The compiler will catch duplicates.

### Where can `mid` appear? Why not inside component definitions?

`mid` is C++-side identity, and C++ addresses *instances* — instances only exist where a component is reused. So `mid` lives exclusively at use-sites:

```tsx
// App.mx — reuse sites: legal
<Counter mid="hero" />
<Counter id="secondary" mid="fives" />   // id (frontend) + mid (C++) coexist
```

```tsx
// Counter.mx — definition side: illegal
export default function Counter(props: { label: string, mid: string }) // hard error: reserved
<div mid="hero">…</div>   // hard error: native elements use id
```

The compiler reads `mid` only when expanding a `<Tag />` instantiation. Declaring it as a prop, putting it on a native element, or using it inside `.map()` templates are all hard errors — each would imply an identity the runtime cannot honor.

### What happens if I delete a component file that has a `mid` tag?

Any native code using that `MID_*` constant will fail to compile — the constant disappears from `morph_api.h`. This is **intentional**: it forces you to update native code when the UI contract changes, instead of silently calling into dead instances (the bug class `mid` was designed to eliminate).

### Why `mid`? Is this really the best approach?

This is the best design we've found given the constraints (zero runtime strings, no registry, no templates, unmount safety, human-readable identity, build-time resolution). If you have a better idea — a different identity model, a simpler DX, a way to handle per-instance native access without the trade-offs we've documented — **please tell us**. The project lives on GitHub; open an issue or PR at [Suggestions](suggestions.md) or email us at [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com). Every design decision in this document is open to challenge — that's how it gets better.

### Can I use `mid` on a component rendered inside a `.map()`?

**Hard error.** List items currently share one state slot per template (documented limitation). Allowing `mid` there would imply per-instance identity that doesn't exist. When per-item state lands, `mid` on list items will use the item's `key` as the sub-address (`MID_HERO::key`), not a positional index.

---

## 11. Related Pages

- [API Reference: morphState](../api/morphState.md) · [morphShared](../api/morphShared.md) · [morphEvent](../api/morphEvent.md) · [morphEffect](../api/morphEffect.md)
- [FAQ: choosing API](../api/faq/choosing.md) · [FAQ: morphShared](../api/faq/morphShared.md) · [FAQ: morphEvent](../api/faq/morphEvent.md)
- [Dev Internals: State & Events](../dev/state/state-events-internals.md) — compiler pipeline details
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
| 2026-09-16 | Generated header named `morph_api.h` (shadows runtime base by include order) | One include for user code; generated header re-includes runtime subpaths explicitly so it is a strict superset |
| 2026-09-16 | Signal/channel definitions live in `morph_api.h` as `inline`, `app.cpp` just includes it | Fixes declaration-before-use for user code included at top of `app.cpp`; TU-safe; `app.cpp` complexity is fine, user DX is sacred |
| 2026-09-16 | `-I<output dir>` before `-I<runtime>` | User `.cpp` `#include "morph_api.h"` resolves to the generated project header |
| 2026-09-16 | `mid` is use-site-only C++ identity (`id` is frontend) | `mid` only on component reuse tags; definition-side declarations, native elements, and `.map()` templates are hard errors; `id`+`mid` coexist on one component |
| 2026-09-16 | No liveness bitmask for `mid` | Build-time statics have no dynamic lifetime; switch-`default` gives the no-op guarantee with zero tracking |
| 2026-09-16 | Native wrappers keep JSX names (`cart()`/`setCart()`) | Same names both sides of the boundary; no `get_`/`set_` prefix dialect |
| 2026-09-16 | `_morph_state.h` drops shared wrappers | `morph_api.h` owns them; duplication was a redefinition error (caught by fixture build) |

---

*End of design record. This page is the source of truth for implementation and docs.*