# Native Interop Test

Demonstrates bidirectional C++/JSX interop via `import { fn } from './file.cpp'`.

## What it tests

- **JSX -> C++** — `doubleIt()` and `area()` are plain C++ functions called directly from JSX event handlers (no FFI, same binary)
- **C++ -> JSX** — `callJsxFromCpp()` calls back into a JSX-defined `jsxHelper()` function
- **C++ -> setState from worker thread** — `runAsync()` updates `morphState` signals from a `std::thread`, proving mutex-protected cross-thread state updates
- **External C++ headers** — `#include <algorithm>` and `<vector>` work alongside user code
- **Shared store via `morph_api.h`** — `resetCartNative()` / `getCartNative()` use the generated `cart()` / `setCart()` / `notify_cartChanged()` wrappers
- **Native event emission** — `announceCartNative()` emits on the same static channel JSX subscribes to (zero string lookup)
- **`mid` instance addressing** — `<Counter mid="hero" />` → `resetHeroCounter()` / `heroCountNative()` via `MID_HERO`

## Files

- `src/App.mx` — JSX entry point importing C++ functions
- `src/CartStore.mx` — `morphShared` cart + `morphEvent` cartChanged
- `src/Counter.mx` — stateful component with a `mid`-tagged instance
- `src/native.cpp` — user C++ code with `doubleIt`, `area`, `callJsxFromCpp`, `runAsync`

## Run

```bash
cd tests/runtime/native-interop
morph dev
# or
morph run
# headless runtime assertions (no display needed):
./.morph/output/native-interop-test --morph-self-test
# or all fixtures at once, from the repo root:
./tests/runtime/run-selftests.sh
```
