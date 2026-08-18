# Build Mode

`morph build` compiles your `.mx` source into an optimized native binary. `morph run` builds and then runs it.

## Building

```bash
morph build          # compile only
morph run            # compile + run
```

## What Happens

```
src/App.mx  →  Pipeline  →  IR dict  →  Feature scan  →  C++ codegen  →  g++  →  binary  →  UPX (optional)
```

1. **Pipeline** — full analysis: parse, walk, CSS cascade, Tailwind, layout, keyframes
2. **Feature scan** — detects which C++ features your app needs (text, flex, hover, animation, etc.)
3. **Code generation** — `Emitter` produces `app.cpp` from Jinja2 templates + the IR
4. **Compilation** — g++ compiles `app.cpp` with the correct flags (FreeType, GLFW, OpenGL, feature defines)
5. **Compression** — optional UPX compression to reduce binary size

## Build Flags

```bash
morph build --static           # statically link GLFW/FreeType/HarfBuzz
morph build --no-upx           # skip UPX compression
morph build --upx-version 4.2  # pin UPX version
morph build --output bin/      # custom output directory
```

## Static Linking

```bash
morph build --static
```

This bundles GLFW, FreeType, and HarfBuzz into a single self-contained binary. The binary has zero external dependencies — it runs on any compatible Linux/macOS system without installing libraries.

Requires the `.a` dev archives. Morph can auto-build FreeType from source if the system archive isn't available.

## UPX Compression

By default, Morph compresses the binary with UPX (a executable compressor). This typically reduces binary size by 50-70%.

```bash
morph build --no-upx    # skip compression
morph build --upx       # force compression (default)
```

Configure in `morph.config.json`:

```json
{
  "build": {
    "upx": true,
    "upx_version": ""
  }
}
```

## Output

The build outputs to the `output` directory (default: `dist/`):

```
dist/
├── app.cpp       ← generated C++ source
└── app           ← compiled binary
```

Build output includes timing and size breakdown:

```
 morph build
──────────────────────────────
 Analyzing source
 Assets & features
   Text rendering (SDF)        ~8-12 KB
   Flexbox layout              ~4-6 KB
   Hover pseudo-class          ~2-3 KB
 Generating C++ code
   app.cpp  45.2 KB  in 120ms
 Compiling binary
──────────────────────────────
 Build complete
   Binary    dist/app  (128.5 KB)
   Code      98.2 KB
   Data      12.1 KB
   Compile   2.3s
   Total     2.8s
```

## Running the Binary

The compiled binary is a standalone executable:

```bash
./dist/app          # run directly
morph run           # build + run
morph run ./my-bin  # run a specific binary
```

Press `Ctrl+C` to stop.

## Platform Requirements

The binary links against:
- **OpenGL 3.3+** (all modern GPUs)
- **GLFW** (window management)
- **FreeType** (text rendering, unless your app has no text)
- **X11** (Linux) or native (macOS)

On Linux, the binary dynamically links against system libraries (unless built with `--static`).
