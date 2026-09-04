# Build Mode

`morphc build` compiles your `.mx` / `.tsx` / `.ts` source into an optimized native binary. `morphc run` builds and then runs it.

## Building

```bash
morphc build          # compile only
morphc run            # compile + run
```

## What Happens

```
src/App.mx  →  Pipeline  →  IR dict  →  Feature scan  →  C++ codegen  →  g++/clang++  →  binary  →  UPX (optional)
```

1. **Pipeline** — full analysis: Oxc parse, lightningcss CSS cascade, Tailwind, layout, keyframes
2. **Feature scan** — detects which C++ features your app needs (text, flex, hover, animation, etc.)
3. **Code generation** — `CppEmitter` produces `app.cpp` from Tera templates + the IR
4. **Compilation** — g++/clang++ compiles `app.cpp` with the correct flags (FreeType, GLFW, OpenGL, feature defines, linker GC sections)
5. **Compression** — optional UPX compression to reduce binary size

## Build Flags

```bash
morphc build --static           # statically link GLFW/FreeType/HarfBuzz
morphc build --no-upx           # skip UPX compression
morphc build --upx-version 4.2  # pin UPX version
morphc build --output bin/      # custom output directory
morphc build --entry src/App.mx # override entry file
```

These override the corresponding fields in `morph.config.json`.

## Static Linking

```bash
morphc build --static
```

This bundles GLFW, FreeType, and HarfBuzz into a single self-contained binary. The binary has zero external dependencies — it runs on any compatible Linux/macOS system without installing libraries.

Requires the `.a` dev archives. Morph can auto-build FreeType from source if the system archive isn't available (controlled by `build.system_freetype` in config).

## UPX Compression

By default, Morph compresses the binary with UPX (an executable compressor). This typically reduces binary size by 50-70%.

```bash
morphc build --no-upx    # skip compression
morphc build --upx       # force compression (default)
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

The build outputs to the `output` directory (default: `.morph/output`, or `dist/` if `morph init` scaffolded it):

```
.morph/output/
├── app.cpp       ← generated C++ source
└── app           ← compiled binary
```

Build output includes timing and size breakdown:

```
 morphc build
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
   Binary    .morph/output/app  (128.5 KB)
   Code      98.2 KB
   Data      12.1 KB
   Compile   2.3s
   Total     2.8s
```

## Incremental Builds

Morph uses content-based fingerprinting to skip unchanged work:

```
Fingerprint = SHA256(
  morph.config.json +
  entry source +
  runtime source tree +
  imported CSS/CPP files
)
```

If the fingerprint matches `.morph/hash/<app>.fingerprint` and the binary exists:

```
 morphc build
 Up to date — nothing to compile
```

## Running the Binary

The compiled binary is a standalone executable:

```bash
./.morph/output/app      # run directly
morphc run               # build + run
morphc run ./my-bin      # run a specific binary
```

Press `Ctrl+C` to stop.

## Platform Requirements

The binary links against:
- **OpenGL 3.3+** (all modern GPUs)
- **GLFW** (window management)
- **FreeType** (text rendering, unless your app has no text)
- **X11/Wayland** (Linux) or native Cocoa (macOS) or Win32 (Windows)

On Linux/macOS, the binary dynamically links against system libraries (unless built with `--static`).