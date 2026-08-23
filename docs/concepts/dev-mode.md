# Dev Mode

`morph dev` starts a development environment with live hot reload. The native window stays open while you edit — changes appear instantly without restarting.

## What Happens

```
File save  →  File watcher  →  Pipeline  →  IR dict  →  Unix socket  →  morph_devrt
                                                  │
                              JS logic  →  logic.<hash>.so  →  dlopen + rewire
```

1. **File watcher** detects a change in your `.mx`, `.css`, or `.ts` files
2. **Pipeline** re-runs: parse, walk, build IR, layout, serialize
3. **JS logic** is translated to C++, compiled to `logic.<hash>.so`, and loaded via `dlopen`. The hot-reload compiler defaults to g++ and can be switched (e.g. to clang++) via `build.dev_cxx` in `morph.config.json` — see [Configuration](../getting-started/configuration.md#compilers).
4. **IR dict** is sent over a Unix socket to `morph_devrt`
5. **Window** swaps the node tree and re-wires signals — no restart needed

## Starting Dev Mode

```bash
morph dev
```

This will:
1. Build the dev runtime binary (`morph_devrt`) via CMake if it doesn't exist
2. Start the Unix socket server
3. Launch the native window
4. Watch for file changes

## Hot Reload

When you save a file:
- The IR is re-sent to the running window
- The node tree swaps instantly
- Your JS logic `.so` is hot-reloaded — signals and effects are re-wired in place
- DevTools state (open/closed, active tab) is preserved

The window never closes. Only the content inside it changes.

## Dev Runtime Binary

`morph_devrt` is a pre-compiled C++ binary that:
- Opens a GLFW window
- Listens on a Unix socket (`/tmp/morph_dev.sock`)
- Receives IR JSON and builds a node tree
- Handles events, layout, and OpenGL rendering
- Supports DevTools (F12)

The binary is auto-built via CMake when first needed. It rebuilds automatically when shared runtime sources change (tracked via a source hash).

## Source Hash Tracking

The dev binary monitors these directories for changes:
- `runtime/dev/`
- `runtime/core/`
- `runtime/render/`
- `runtime/ui/`
- `runtime/style/`
- `runtime/renderers/`

If any file in these directories changes, `morph_devrt` is rebuilt before the next dev session.

## DevTools

Press **F12** to toggle the DevTools panel. It includes:

- **Elements** — inspect element tree, box model overlay, element info
- **Rendering** — frame stats, layout/paint diagnostics, live renderer switch
- **Network** — `fetch()` request log
- **Logs** — application log entries

See the [DevTools](../devtools/index.md) section for details on each tab.

## Tips

- **Wayland issues** — If the window doesn't open, try: `GDK_BACKEND=x11 morph dev`
- **Slow reload** — Check if your CSS files are large or if you have many Tailwind classes
- **Socket errors** — Delete `/tmp/morph_dev.sock` and restart
- **Binary missing** — Run `morph doctor` to verify cmake, g++, and make are installed
