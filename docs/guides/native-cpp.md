# C++ / JSX Interop

Import user C++ functions directly into your JSX code. No FFI, no bindings — the C++ is compiled into the same binary.

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

Use the imported functions in event handlers:

```tsx
<button onClick={() => setCount(doubleIt(count))}>Double it</button>
<button onClick={() => setArea(area(5, 7))}>Compute area</button>
```

## How It Works

1. Morph detects `import { fn } from './file.cpp'` in your `.mx` file
2. The user `.cpp` file is `#included` into the generated translation unit
3. Functions are callable directly from JSX event handlers
4. A `_morph_state.h` header is generated exposing signals and setter wrappers so C++ can update state and call JSX code

## C++ → JSX State

C++ code can update `morphState` signals from any thread:

```cpp
#include "morph_api.h"

void loadData() {
    setStatus("loading...");
    std::thread([]() {
        // ... do work ...
        setData("result");
        setStatus("done");
    }).detach();
}
```

The `setStatus()` and `setData()` functions are generated wrappers that update the corresponding morphState signals. They are mutex-protected and safe to call from worker threads.

## Clipboard

Native code (and `<input>` copy/paste) uses the GLFW-backed clipboard API from `morph/runtime/core/clipboard.h`:

```cpp
#include "clipboard.h"

void copyGreeting() {
    morph::setClipboard("Hello from Morph");
}

std::string paste = morph::getClipboard();
```

## C++ → JSX Function Calls

C++ can call functions defined in your JSX:

```tsx
// src/App.mx
function jsxHelper(x: int): int {
  return x * 10 + 1
}
```

```cpp
// C++ can call jsxHelper via the generated header
#include "_morph_state.h"

int callJsxFromCpp(int x) {
    return jsxHelper(x);
}
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

## Example

```tsx
import { CSS, morphState } from 'morph'
import { doubleIt, area, runAsync } from './native.cpp'

CSS.load("style.css")

export const windowConfig = { title: "Interop", width: 640, height: 400 }

export default function App() {
  const [count, setCount] = morphState(0)
  const [status, setStatus] = morphState("idle")

  return (
    <body>
      <div>Count: {count}</div>
      <button onClick={() => setCount(doubleIt(count))}>Double it</button>
      <button onClick={() => runAsync(count)}>Run async</button>
      <div>{status}</div>
    </body>
  )
}
```

See the [native-interop test](../../tests/runtime/native-interop/README.md) for a complete working example.
