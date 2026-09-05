# Debugging & Profiling

Tools and techniques for debugging Morph applications.

## Debug Builds

```bash
# Dev mode already builds with debug info
morph dev

# Explicit debug build
morph build --entry src/App.mx
# Output has DWARF debug info (no -s flag)
```

## GDB / LLDB

```bash
# Linux/macOS
gdb .morph/output/my-app
# or
lldb .morph/output/my-app

# Run with args
(gdb) run --width 800 --height 600

# Breakpoints
(gdb) break morph::ButtonNode::onClick
(gdb) break 'morph::Signal<int>::set'
(gdb) break main

# Print Morph types
(gdb) p my_signal.value_
(gdb) p my_js_string.value
(gdb) p my_js_number.value

# Backtrace on crash
(gdb) bt
```

### Pretty Printers

Add to `~/.gdbinit` for better `JsValue`/`JsString`/`JsNumber` display:

```python
# ~/.gdbinit
import gdb

class JsStringPrinter:
    def __init__(self, val):
        self.val = val
    def to_string(self):
        return self.val['value'].string()

class JsNumberPrinter:
    def __init__(self, val):
        self.val = val
    def to_string(self):
        v = self.val['value']
        if v.type.tag == 'T_int64_t':
            return str(v['int64_t'])
        elif v.type.tag == 'T_double':
            return str(v['double'])
        else:
            return v['string'].string()

class JsValuePrinter:
    def __init__(self, val):
        self.val = val
    def to_string(self):
        inner = self.val['inner']
        which = inner['_M_index']
        if which == 0: return "undefined"
        if which == 1: return "null"
        if which == 2: return f"bool({inner['_M_storage']['_M_value']['value']})"
        if which == 3: return f"number({JsNumberPrinter(inner['_M_storage']['_M_value']).to_string()})"
        if which == 4: return f"string({JsStringPrinter(inner['_M_storage']['_M_value']).to_string()})"
        if which == 5: return "array(...)"
        if which == 6: return "object(...)"
        return "unknown"

gdb.pretty_printers.append(lambda val: JsStringPrinter(val) if str(val.type) == 'morph::JsString' else None)
gdb.pretty_printers.append(lambda val: JsNumberPrinter(val) if str(val.type) == 'morph::JsNumber' else None)
gdb.pretty_printers.append(lambda val: JsValuePrinter(val) if str(val.type) == 'morph::JsValue' else None)
```

## Visual Studio Debugger (Windows)

1. Open `morph.sln` or create a VS project pointing to `.morph/output/my-app.exe`
2. Debug → Start Debugging (F5)
3. Use **Watch** window for `my_signal.value_`, `my_js_string.value`
4. **Natvis** for Morph types (create `.natvis` file):

```xml
<?xml version="1.0" encoding="utf-8"?>
<AutoVisualizer xmlns="http://schemas.microsoft.com/vstudio/debugger/natvis/2010">
  <Type Name="morph::JsString">
    <DisplayString>{value,sb}</DisplayString>
    <Expand>
      <Item Name="length">value.size()</Item>
      <Item Name="capacity">value.capacity()</Item>
    </Expand>
  </Type>
  <Type Name="morph::JsNumber">
    <DisplayString Condition="value.index() == 0">int64: {value._M_storage._M_value.int64_t}</DisplayString>
    <DisplayString Condition="value.index() == 1">double: {value._M_storage._M_value.double}</DisplayString>
    <DisplayString Condition="value.index() == 2">bigint: {value._M_storage._M_value.string,sb}</DisplayString>
  </Type>
</AutoVisualizer>
```

## DevTools (F12 in Dev Mode)

| Tab | Use For |
|---|---|
| **Elements** | Inspect node tree, box model, computed styles, event listeners |
| **Rendering** | Frame time, layout/paint stats, culling, renderer toggle (Flash/Forge) |
| **Network** | `fetch()` requests: status, headers, body, timing |
| **Logs** | `console.log/warn/error/info`, runtime errors, hot-reload status |

### Console Logging

```tsx
console.log("plain", value);           // Logs tab, level=info
console.warn("warning");               // Logs tab, level=warn (yellow)
console.error("error", err);           // Logs tab, level=error (red)
console.info("debug info");            // Logs tab, level=info (blue)

// In C++ (native code)
#include "dev/dev_log.h"
MORPH_LOG_INFO("Native: {}", value);
MORPH_LOG_WARN("Native warning");
MORPH_LOG_ERROR("Native error: {}", err);
```

## Profiling

### CPU Profiling (Linux)

```bash
# perf
perf record -g --call-graph=dwarf .morph/output/my-app
perf report

# Or with flamegraph
perf script | ./FlameGraph/stackcollapse-perf.pl | ./FlameGraph/flamegraph.pl > profile.svg
```

### CPU Profiling (macOS)

```bash
# Instruments
xcrun xctrace record --template "Time Profiler" --launch -- .morph/output/my-app

# Or sample
sample .morph/output/my-app 5 -f profile.txt
```

### Memory Profiling

```bash
# Valgrind (Linux)
valgrind --tool=massif --massif-out-file=massif.out .morph/output/my-app
ms_print massif.out

# Heaptrack (Linux)
heaptrack .morph/output/my-app
heaptrack_gui heaptrack.my-app.gz

# macOS
leaks -atExit -- .morph/output/my-app
```

### GPU Profiling

```bash
# RenderDoc (cross-platform)
renderdoc .morph/output/my-app

# NVIDIA Nsight (Linux/Windows)
nsight-cli .morph/output/my-app

# macOS Metal debugger (Forge renderer)
# Xcode → Debug → Capture GPU Frame
```

## Hot Reload Debugging

Dev mode compiles `logic.<hash>.so` on each save. To debug the shared library:

```bash
# Attach to running morph_devrt
gdb -p $(pidof morph_devrt)

# Set breakpoint in your logic (symbols loaded from logic.so)
(gdb) break App.tsx:42
# Or by mangled name
(gdb) break _ZN4morph10ButtonNode7onClickEv
```

## Common Issues

| Symptom | Cause | Fix |
|---|---|---|
| Segfault on startup | Missing runtime, wrong GL version | `morph doctor`, check `glxinfo \| grep OpenGL` |
| Blank window | Forge renderer bug | Switch to `flash` in `morph.config.json` |
| Text not rendering | FreeType not found | `morph doctor -y`, check `build.system_freetype` |
| Hot reload not working | Socket permission / stale `.morph/dev.sock` | `rm .morph/dev.sock && morph dev` |
| `morph build` hangs | CMake configure (first run) | Wait or run `morph doctor` first |
| Linker errors | Missing `-l` flags for C++ imports | Add to `native.libraries` in config |

## Debug Flags

```bash
# Verbose compiler output
MORPH_CXX="g++ -v" morph build

# Enable runtime debug logs
MORPH_LOG=debug morph dev

# Disable compiler optimizations for debugging
# Edit morph.config.json:
{ "build": { "cxx": "g++", "cflags": ["-O0", "-g3"] } }
```

## Assertions & Sanitizers

```bash
# AddressSanitizer (detect buffer overflows, use-after-free)
morph build --entry src/App.mx
# Then manually compile with ASan:
g++ -fsanitize=address -g -O1 app.cpp runtime_sources... -o my-app-asan

# ThreadSanitizer (detect data races)
g++ -fsanitize=thread -g -O1 app.cpp runtime_sources... -o my-app-tsan

# Run
./my-app-asan
./my-app-tsan
```

## Symbol Server / Source Indexing

For CI builds, upload debug symbols:

```bash
# Linux: Build with -gsplit-dwarf, upload .dwo files
# macOS: dSYM bundles
dsymutil .morph/output/my-app -o my-app.dSYM
# Upload to Sentry / Symbolicator / internal server
```

## Useful Breakpoints

```gdb
# Layout/paint
break morph::LayoutEngine::layout
break morph::Flatten::flatten

# Reactivity
break morph::SignalBase::notify
break morph::create_effect

# Events
break morph::Window::onMouseButton
break morph::Window::onKey

# Network
break morph::net::HttpAwaitable::await_resume
break morph::net::fetch

# Coroutines
break morph::Result<int>::promise_type::return_value
break morph::Task::promise_type::return_void
```