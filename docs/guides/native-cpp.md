# C++ / JSX Interop

Import user C++ functions directly into your JSX code. No FFI, no bindings — the C++ is compiled into the same binary.

One include gives native code everything: `#include "morph_api.h"`. That header is generated per project (next to `app.cpp`) and is the **entire** native contract — you never read `app.cpp`.

## Basic Usage

Create a `.cpp` file and import its functions:

```tsx
// src/App.mx
import { doubleIt, area } from './native.cpp'
```

```cpp
// src/native.cpp
int doubleIt(int x) {
    return x * 2;
}

double area(double w, double h) {
    return w * h;
}
```

```tsx
<button onClick={() => setCount(doubleIt(count))}>Double it</button>
```

## How It Works

1. Morph detects `import { fn } from './file.cpp'` in your `.mx` file
2. The user `.cpp` file is `#included` into the generated translation unit
3. Functions are callable directly from JSX event handlers
4. `morph_api.h` is generated with signal/channel definitions plus thin wrappers, so C++ can read/write state, emit events, and address tagged instances

## Generated Headers

User C++ includes one header: `morph_api.h`. Two headers are generated per project, split by ownership:

- **`_morph_state.h`** — window state and its accessors: `extern __st_*` signal declarations plus the `morphState` wrappers (`app::<ns>::count()` / `app::<ns>::setCount(v)`). The entry module's own state lives at `app::app::`.
- **`morph_api.h`** — module bindings native code can discover: shared-store accessors, event channels + `emit_`/`notify_` wrappers, `MID_*` constants with indexed `setCount(mid, v)`/`count(mid)` accessors, and module function/var/class declarations.

`_morph_state.h` includes `morph_api.h`, and the generated TU includes both — user C++ never includes `_morph_state.h` directly and never reads `app.cpp`.

Generated code spells references absolute (`::app::cartstore::...`): inside namespace blocks a leading `app::` would resolve through `app::app`. Your own C++ at global scope can write plain `app::...`; use the `::app::` form if you nest code in your own namespaces.

### Styling from C++

Style keyword fields are scoped enums, not strings — assign enum literals from native code:

```cpp
node->style.display = CSS::Display::Flex;
node->style.position = CSS::Position::Absolute;
node->type = NodeType::Button;
```

A string assignment (`node->style.display = "flex"`) fails at compile time pointing at the line. Parse helpers (`CSS::parseDisplay(...)`, `parseNodeType(...)`) exist for string input; `toString(...)` prints back on debug paths.

### Dev mode (`morph dev`)

`morph dev` regenerates `morph_api.h` on **every rebuild** — same namespaces and wrapper names as `morph build`, bound to dev-TU definitions (string-registry channels, TU-local signals) instead of static ones. It is written to both `.morph/cache/` (what the dev logic TU compiles against) and `.morph/output/` (the stable path below), so bodies may differ from a build depending on which flow ran last — declarations never do.

Point your IDE at `.morph/output/morph_api.h` for suggestions: it stays fresh without ever running `morph build`.

Full parity in dev: the dev TU includes user C++ after all generated definitions, so native code sees module classes, vars, and functions directly, and `morph_api.h` carries the re-export aliases. Event channels stay string-registry-based under the hood (same call syntax).

## Decision Tree

| Need | Route | Code |
|---|---|---|
| Read shared state | `<binding>()` wrapper | `app::cartstore::cart()` |
| Write shared state | `<setter>(v)` + optional `notify_<event>()` | `setCart(0); notify_cartChanged();` |
| Emit event | `emit_<name>(payload)` or `evt_<name>().emit(...)` | `emit_cartChanged(JsObject{{"cart", cart()}});` |
| Call a TS/TSX function | Defining module's namespace (never the importer's) | `app::utility::loadData()` |
| Call a re-exported function | Re-exporting namespace (via `using`-alias) | `app::mid::loadData()` |
| Read another component's local | **Don't.** Emit an event; let that component reset its own local | `evt_resetEvent().emit({});` / `resetEvent.on(() => setCount(0))` |
| C++ produces a value for a local | Return it; the TSX handler writes its own local | `int compute();` / `<button onClick={() => setCount(compute())}>` |
| **Native initiates a write to a specific instance** | Tag `<Comp mid="tag" />`; use the `MID_TAG` constant | `setCount(MID_HERO, 0)` |
| Pure compute | Plain function, zero plumbing, always | `int doubleIt(int x) { return x * 2; }` |

## Calling TS / TSX Bindings from C++

Every binding lives in its defining file's namespace — find it in `morph_api.h` via the mapping comments:

```cpp
#include "morph_api.h"

int loadFromNative() {
    return app::utility::loadData();   // defined in utility.ts
}

int viaReexport() {
    return app::mid::loadData();       // mid.ts re-exports utility.ts
}

void makeUser() {
    app::models::User u("hero");       // defined in models.ts
}
```

Rules: import any exported binding (`import { loadData } from './utility.ts'`, default or named) and call it bare in JSX — the compiler rewrites to the defining namespace. Unknown or ambiguous imports are hard errors. Same-name definitions in different files coexist; each keeps its own namespace.

## Shared State (`morphShared`)

```tsx
// src/CartStore.mx
export const [cart, setCart] = morphShared<number>(0)
```

```cpp
#include "morph_api.h"

void resetCartNative() {
    app::cartstore::setCart(0);           // wrapper → shared_cart().set(0)
    app::cartstore::notify_cartChanged(); // wrapper → evt_cartChanged().emit({})
}

int getCartNative() {
    return app::cartstore::cart();        // wrapper → shared_cart().get()
}
```

Namespaces are human-computable from file paths: `CartStore.mx` next to the entry → `app::cartstore`; `src/components/shop/ShopStore.mx` → `app::components::shop::shopstore`. Wrapper names match the JSX bindings exactly.

## Events (`morphEvent`)

```tsx
// src/CartStore.mx
export const cartChanged = morphEvent<{ cart: number }>()
```

```cpp
// Emit from anywhere (thread-safe; listeners run on the emitter's thread):
app::cartstore::emit_cartChanged(JsObject{{"cart", app::cartstore::cart()}});

// Or the channel directly:
app::cartstore::evt_cartChanged().emit(JsObject{{"cart", 3}});
```

Zero string lookup: each event is one static `Channel` in its module namespace. Subscribe from JSX with `cartChanged.on(...)` — user C++ never subscribes (generated code owns subscriptions).

## Specific Instances (`mid`)

Tag the instances native code may address — untagged instances stay purely local (no overhead, no access):

```tsx
<Counter mid="hero" />
<Counter />
```

```cpp
void resetHeroCounter() {
    app::counter::setCount(app::counter::MID_HERO, 0);
}

int heroCountNative() {
    return app::counter::count(app::counter::MID_HERO);
}
```

Rules: `mid` is a string literal (`mid="hero"`, never `mid={x}`), letters-only (`[a-zA-Z]`, any case), unique per component type (case-insensitive), reserved (never a prop), and rejected inside `.map()` item templates. `mid` exists only at component reuse sites — never inside a component definition, never on native elements (frontend identity is `id`; the two coexist: `<Comp id="card" mid="hero" />`). Unknown indexes are silent no-ops (switch dispatch, bounds-safe by construction).

`morph_api.h` carries a mapping comment per constant (`// <Counter mid="hero"> (App.mx:5:7)`), and deleting a tagged component breaks native compilation loudly — the UI contract changing forces native code to follow.

## C++ → JSX State (any thread)

`set...()` wrappers are mutex-protected; effects re-run on the main loop.
Every call is namespace-qualified — the entry module's own state lives at
`app::app::` (entry `App.mx` → namespace `app` under root `app::`):

```cpp
void runAsync(int start) {
    app::app::setStatus("working...");
    std::thread([start]() {
        std::this_thread::sleep_for(std::chrono::milliseconds(60));
        app::app::setCount(start + 100);
        app::app::setStatus("done");
    }).detach();
}
```

## C++ → JSX Function Calls

```tsx
// src/App.mx
function jsxHelper(x: int): int {
  return x * 10 + 1
}
```

```cpp
// Declared in the generated headers; defined in the generated TU.
int callJsxFromCpp(int x) {
    return app::app::jsxHelper(x);
}
```

## Clipboard

Native code (and `<input>` copy/paste) uses the GLFW-backed clipboard API from `runtime/cpp/core/clipboard.h`:

```cpp
#include "clipboard.h"

void copyGreeting() {
    morph::setClipboard("Hello from Morph");
}

std::string paste = morph::getClipboard();
```

## Configuration

Set build options for your C++ imports in `morph.config.json`:

```json
{
  "native": {
    "include_dirs": ["libs/include"],
    "library_dirs": ["libs/lib"],
    "libraries": ["png", "z"],
    "cflags": ["-O3"],
    "ldflags": []
  }
}
```

See [Configuration](../getting-started/configuration.md) for all options.

## Verifying (`--morph-self-test`)

Every build embeds headless runtime assertions (shared roundtrips, event delivery, `mid` indexed writes). Run without a display:

```sh
./.morph/output/<app> --morph-self-test
# [morph-self-test] 3 checks, 0 failures
```

The [runtime self-test script](../../tests/runtime/run-selftests.sh) rebuilds the fixtures and runs this automatically.

## Example

```tsx
import { morphState } from 'morph'
import { doubleIt, area, runAsync, resetCartNative } from './native.cpp'
import { cart } from './CartStore.mx'
import Counter from './Counter.mx'
import "./style.css"

export const windowConfig = { title: "Interop", width: 640, height: 400 }

export default function App() {
  const [count, setCount] = morphState(0)
  const [status, setStatus] = morphState("idle")

  function addToCart() { /* shared store lives in CartStore.mx */ }

  return (
    <body>
      <div>Count: {count}</div>
      <button onClick={() => setCount(doubleIt(count))}>Double it</button>
      <button onClick={() => runAsync(count)}>Run async</button>
      <div>{status}</div>
      <div>Cart: {cart}</div>
      <Counter mid="hero" />
      <Counter />
    </body>
  )
}
```

See the [native-interop fixture](../../tests/runtime/native-interop/src/App.mx) for a complete working example.
