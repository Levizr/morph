# How Do I Debug a Morph App?

## DevTools (Fastest Path)

In dev mode, press **F12**:

| Tab | Use For |
|---|---|
| **Elements** | Node tree, box model, computed styles |
| **Rendering** | Frame time, layout/paint stats, Flash/Forge toggle |
| **Network** | `fetch()` requests: status, headers, body |
| **Logs** | `console.log/warn/error/info`, hot-reload status |

## Native Debuggers

```bash
# Linux/macOS
gdb .morph/output/my-app
lldb .morph/output/my-app

# Useful breakpoints
(gdb) break morph::SignalBase::notify
(gdb) break morph::Window::onMouseButton
(gdb) break morph::net::HttpAwaitable::await_resume
(gdb) bt   # backtrace on crash
```

## Profiling

```bash
# CPU (Linux)
perf record -g .morph/output/my-app && perf report

# Memory (Linux)
valgrind --tool=massif .morph/output/my-app
heaptrack .morph/output/my-app

# GPU
renderdoc .morph/output/my-app
```

## Common Issues

| Symptom | Fix |
|---|---|
| Blank window | Switch `renderer` to `"flash"` in config (Forge is beta) |
| Hot reload stuck | `rm .morph/dev.sock && morph dev` |
| Text missing | `morph doctor -y`, check `build.system_freetype` |
| Linker errors | Add libs to `native.libraries` in config |

See the full [Debugging & Profiling guide](../guides/debugging.md) for GDB pretty printers, Natvis, sanitizers (ASan/TSan), and hot-reload debugging.