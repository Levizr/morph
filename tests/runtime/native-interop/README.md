# Native Interop Test

Demonstrates bidirectional C++/JSX interop via `import { fn } from './file.cpp'`.

## What it tests

- **JSX -> C++** — `doubleIt()` and `area()` are plain C++ functions called directly from JSX event handlers (no FFI, same binary)
- **C++ -> JSX** — `callJsxFromCpp()` calls back into a JSX-defined `jsxHelper()` function
- **C++ -> setState from worker thread** — `runAsync()` updates `morphState` signals from a `std::thread`, proving mutex-protected cross-thread state updates
- **External C++ headers** — `#include <algorithm>` and `<vector>` work alongside user code

## Files

- `src/App.mx` — JSX entry point importing C++ functions
- `src/native.cpp` — user C++ code with `doubleIt`, `area`, `callJsxFromCpp`, `runAsync`

## Run

```bash
cd tests/runtime/native-interop
morph dev
# or
morph run
```
