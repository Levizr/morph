# Why Two Generated Headers? (`_morph_state.h` vs `morph_api.h`)

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

Every `morph build` generates two C++ headers next to `app.cpp`. They split by ownership — here's why, and which one you touch.

## Which header do I include in my C++?

One: `morph_api.h`. That's the entire native contract. You never include `_morph_state.h` directly, and you never read `app.cpp`.

```cpp
#include "morph_api.h"

void runAsync(int start) {
    app::app::setStatus("working..."); // entry state — always qualified
}
```

## Why does `_morph_state.h` exist at all? Why not put everything in `morph_api.h`?

`_morph_state.h` is the older header — it predates `morph_api.h`. It owns **window state and its accessors**: the `extern __st_*` signal declarations plus the `morphState` wrappers (`count()` / `setCount(v)`). It stays separate for three reasons:

1. **A wrapper wraps a signal declared in the same header.** Moving `setCount` into `morph_api.h` means moving (or duplicating) the `extern morph::Signal<int> __st_count;` declarations they reference. One header owns the signal *and* its accessor — no cross-header coupling. (`_morph_state.h` already includes `morph_api.h`, so the dependency runs one way.)
2. **Flow coverage differs.** The generated app TU includes both, but only the *build* flow generates `morph_api.h` — the dev flow never does, so module-function declarations live qualified in `_morph_state.h` to keep native code compiling everywhere. Root-state wrappers in `_morph_state.h` work in every flow with zero special-casing.
3. **Layering.** `_morph_state.h` = "window state and its accessors". `morph_api.h` = "module bindings native code can discover" (shared stores, event channels, `MID_*` + indexed accessors, module function/var/class declarations).

Summary:

| Header | Owns | You include it? |
|---|---|---|
| `_morph_state.h` | `extern __st_*` signals + `morphState` wrappers | No (generated TU does) |
| `morph_api.h` | Shared/event/`mid`/module bindings | **Yes — the only one** |

## Why is entry state at `app::app` — double `app`?

The first `app` is the generated-code root namespace; the second is the entry module's own namespace (`App.mx` → `app`, same human-computable rule as every file: `CartStore.mx` → `app::cartstore`). So the entry's `count` is `app::app::count()`. Uniform rule, no special cases — the [`C++ / JSX Interop`](../guides/native-cpp.md) guide covers it.

## Why doesn't the entry component's own state support `mid`?

`mid` addresses tagged component **uses** (`<Counter mid="hero" />`), and the entry component is instantiated by the runtime — never by JSX. It has no use-site, so there is nothing to tag: its `morphState` is a singleton with plain wrappers (`app::app::count()` / `app::app::setCount(v)`), no `MID_*`, no indexed form. This is structural, not a missing feature — root state can never pass through instance expansion.

Related: a component *defined* in `App.mx` but *used* with `mid` gets its constants under its own component scope (`app::<comp>`, e.g. `app::foo` for `Foo`) — not bare `app::app`. Only a component literally named `App` would land directly in `app::app`.

## Can I call these wrappers from any thread?

Yes. `set...()` wrappers are mutex-protected; effects re-run on the main loop — no UI-thread hopping needed. See [C++ → JSX State](../guides/native-cpp.md#c--jsx-state-any-thread).
