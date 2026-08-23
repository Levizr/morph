# Configuration

All project settings live in `morph.config.json` at the project root.

## Full Reference

```json
{
  "name": "my-app",
  "entry": "src/App.mx",
  "output": "dist/",
  "window": {
    "width": 800,
    "height": 600,
    "title": "Morph App"
  },
  "renderer": "flash",
  "dependencies": {},
  "cpp_sources": [],
  "native": {
    "include_dirs": [],
    "library_dirs": [],
    "libraries": [],
    "cflags": [],
    "ldflags": []
  },
  "build": {
    "wayland": false,
    "system_freetype": false,
    "upx": true,
    "upx_version": "",
    "cxx": "",
    "dev_cxx": "",
    "cmake": ""
  },
  "lint": {
    "disable": [],
    "severities": {}
  }
}
```

## Top-Level Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | `"my-app"` | Project name. Used in window title and build output. |
| `entry` | string | `"src/App.mx"` | Root `.mx` file to compile. |
| `output` | string | `"dist/"` | Directory for build artifacts (`morph init` scaffolds `dist/`; omitting the key falls back to `.morph/`). |
| `window` | object | (see below) | Native window settings. |
| `renderer` | string | `"flash"` | Renderer backend: `"flash"` (default, lightweight) or `"forge"` (retained surfaces). |
| `dependencies` | object | `{}` | Package dependencies (key-value pairs). |
| `cpp_sources` | array | `[]` | C++ source files to include in the build. |
| `native` | object | (see below) | Build options for imported C++ files. |
| `build` | object | (see below) | Static linking and compression settings. |
| `lint` | object | (see below) | Lint rule overrides for `.mx` files. |

## `window`

| Field | Type | Default | Description |
|---|---|---|---|
| `width` | int | `800` | Window width in CSS pixels. |
| `height` | int | `600` | Window height in CSS pixels. |
| `title` | string | `"Morph App"` | Window title bar text. |

Window size limits (`minWidth`, `minHeight`, `maxWidth`, `maxHeight`) are not read from `morph.config.json` — set them via the `windowConfig` export or `<morph-window>` props instead (below).

Window settings can also be overridden per-file by exporting `windowConfig` from your entry `.mx`:

```tsx
export const windowConfig = {
  title: "Calculator",
  width: 340,
  height: 500,
  minWidth: 340,
  minHeight: 500,
  maxWidth: 340,
  maxHeight: 500,
}
```

## `native`

Options forwarded to g++ when importing user C++ files (`import { fn } from './file.cpp'`).

| Field | Type | Default | Description |
|---|---|---|---|
| `include_dirs` | string[] | `[]` | Additional `-I` paths for headers. |
| `library_dirs` | string[] | `[]` | Additional `-L` paths for archives. |
| `libraries` | string[] | `[]` | Libraries to link (`-l` flags without the `-l` prefix). |
| `cflags` | string[] | `[]` | Extra compile flags (e.g. `["-O3"]`). |
| `ldflags` | string[] | `[]` | Extra link flags. |

Example:

```json
{
  "native": {
    "include_dirs": ["libs/include"],
    "libraries": ["png", "z"],
    "cflags": ["-O3", "-march=native"]
  }
}
```

## `build`

| Field | Type | Default | Description |
|---|---|---|---|
| `wayland` | bool | `false` | Enable GLFW Wayland backend (adds ~150 KB + deps). |
| `system_freetype` | bool | `false` | Use system `libfreetype.a` instead of the trimmed self-built copy. |
| `upx` | bool | `true` | Compress the final binary with UPX. |
| `upx_version` | string | `""` | Pin a specific UPX release (e.g. `"4.2.4"`). Empty uses system/default. |
| `cxx` | string | `""` | Compiler for production binaries (`morph build`). Empty = auto (`g++`). |
| `dev_cxx` | string | `""` | Compiler for dev-mode hot-reload logic. Empty = auto (newest `clang++`, else `g++`). |
| `cmake` | string | `""` | CMake binary used to build the dev runtime. Empty = `"cmake"` on PATH. |

### Compilers

Morph uses two independently configurable toolchains:

- **Production (`morph build`, `morph run`)** — uses `build.cxx` if set, otherwise the platform default `g++`. Release binaries have one predictable toolchain.
- **Dev hot reload (`morph dev`)** — every logic edit recompiles a small `.so`. The default is also `g++`: on our benchmarks (GCC 14 / libstdc++) it parses morph's template-heavy runtime headers measurably faster than clang (~3.5s vs ~4.4s for a small app). If your toolchain differs you can opt into clang with one line:

```json
{
  "build": {
    "cxx": "/usr/bin/g++-13",
    "dev_cxx": "/usr/bin/clang++-19",
    "cmake": "/snap/bin/cmake"
  }
}
```

The same values can be set per-session with environment variables (config wins over env, env wins over defaults):

```sh
export MORPH_CXX=/usr/bin/g++-13
export MORPH_DEV_CXX=clang++-17
export MORPH_CMAKE=/usr/local/bin/cmake
```

A configured path that doesn't exist falls back to the platform default with a warning rather than failing the build.

**Faster dev linker (optional):** the hot-reload link step is small (~0.2s), but if you want it anyway, add a faster linker through the existing native flags:

```json
{
  "native": {
    "ldflags": ["-fuse-ld=mold"]
  }
}
```

## `lint`

`.mx` files are checked against the framework's supported surface (elements, props, CSS properties, JS API). Errors block `morph build` and dev-mode reloads (Next.js-style — dev keeps watching and hot-reloads once fixed); warnings are reported only. Run `morph check` for a lint-only pass.

| Field | Type | Default | Description |
|---|---|---|---|
| `disable` | string[] | `[]` | Rule codes to turn off entirely, e.g. `["mx-list-key"]`. |
| `severities` | object | `{}` | Rule code → `"error"` / `"warning"` overrides, e.g. `{"mx-tag": "warning"}`. |

Example:

```json
{
  "lint": {
    "disable": ["mx-list-key"],
    "severities": { "mx-tag-stub": "error" }
  }
}
```

### Rule codes

| Code | Checks | Default |
|---|---|---|
| `mx-export` | Exactly one `export default function` component | error |
| `mx-component-name` | Default export is a named function | error |
| `mx-window-conflict` | `<morph-window>` and `windowConfig` used together | error |
| `mx-window-missing` | No window is created (no `morph-window` / `windowConfig`) | error |
| `mx-windowconfig-key` / `mx-windowconfig-type` | Valid `windowConfig` keys and types | error |
| `mx-window-prop` / `mx-window-prop-type` | Valid `<morph-window>` props and types | error |
| `mx-tag` | Element is in the supported set | error |
| `mx-tag-stub` | `<input>` / `<select>` / `<textarea>` not fully implemented | warning |
| `mx-prop` | Prop is valid for the element | error |
| `mx-img-src` | `<img>` has a `src` | error |
| `mx-event-value` | Event handler is a function | error |
| `mx-morph-action` | `morph-*` value is a static string | error |
| `mx-key-misuse` | `key` only inside `.map()` lists | warning |
| `mx-dup-class` | `className` and `class` not both used | warning |
| `mx-style-prop` | Inline style key is supported | error |
| `mx-style-value` | Inline style value is valid | warning |
| `mx-tailwind-class` | `className` token resolves to a Tailwind class | warning |
| `mx-css-prop` | CSS file property is supported | warning |
| `mx-css-file-missing` | Imported CSS file exists | error |
| `mx-import-morph` | Imported name is exported by `morph` | error |
| `mx-import-missing` | Imported file exists | error |
| `mx-import-type` | Import is `.css` / `.cpp` / `morph` | warning |
| `mx-state-pattern` | `morphState` destructured as `[getter, setter]` | error |
| `mx-effect-cb` | `morphEffect` first arg is a function | error |
| `mx-effect-deps` | Effect deps are state variables | warning |
| `mx-transpile` | JS compiles to C++ — JSX expressions, event/effect bodies, component consts, inner functions, global vars, top-level functions | error |
| `mx-js-global` | Browser/JS globals with no native counterpart (`document`, `window`, `localStorage`, `Math`, `JSON`, `Date`, `alert`, `parseInt`, …) | error |
| `mx-js-member` | Member access on unsupported JS builtins (`Math.*`, `JSON.*`, `Date.*`, `RegExp.*`, `console.*` other than log/warn/error/info) | error |
| `mx-js-method` | Methods the runtime types don't implement (`.map()`/`.split()`/`.toUpperCase()` on state values or string literals, …) | error |
| `mx-js-op` | Operators the C++ translator can't emit (`typeof`, `**`, `instanceof`, `in`, `delete`, `??=`, `&&=`, `||=`, `>>>`, `void`) | error |
| `mx-js-syntax` | JS constructs the translator can't handle (`?.`, destructuring, generators/`yield`, `for...in`/`for...of`, object spread, spread in call args, object methods/getters) | error |
| `mx-list-key` | `.map()` item root has a `key` | warning |

## Build Flags (CLI)

These `morph.config.json` fields can be overridden via CLI flags on `morph build` and `morph run`:

```bash
morph build --static           # statically link GLFW/FreeType/HarfBuzz
morph build --no-upx           # skip UPX compression
morph build --upx-version 4.2  # pin UPX version
morph build --output bin/      # custom output directory
morph build --entry src/App.mx # override entry file
```
