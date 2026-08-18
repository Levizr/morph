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
    "title": "My App",
    "minWidth": 400,
    "minHeight": 300,
    "maxWidth": 1920,
    "maxHeight": 1080
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
    "upx_version": ""
  }
}
```

## Top-Level Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | string | `"my-app"` | Project name. Used in window title and build output. |
| `entry` | string | `"src/App.mx"` | Root `.mx` file to compile. |
| `output` | string | `"dist/"` | Directory for build artifacts. |
| `window` | object | (see below) | Native window settings. |
| `renderer` | string | `"flash"` | Renderer backend: `"flash"` (default, lightweight) or `"forge"` (retained surfaces). |
| `dependencies` | object | `{}` | Package dependencies (key-value pairs). |
| `cpp_sources` | array | `[]` | C++ source files to include in the build. |
| `native` | object | (see below) | Build options for imported C++ files. |
| `build` | object | (see below) | Static linking and compression settings. |

## `window`

| Field | Type | Default | Description |
|---|---|---|---|
| `width` | int | `800` | Window width in CSS pixels. |
| `height` | int | `600` | Window height in CSS pixels. |
| `title` | string | `"My App"` | Window title bar text. |
| `minWidth` | int | — | Minimum window width (enforced by OS). |
| `minHeight` | int | — | Minimum window height (enforced by OS). |
| `maxWidth` | int | — | Maximum window width (enforced by OS). |
| `maxHeight` | int | — | Maximum window height (enforced by OS). |

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

## Build Flags (CLI)

These `morph.config.json` fields can be overridden via CLI flags on `morph build` and `morph run`:

```bash
morph build --static           # statically link GLFW/FreeType/HarfBuzz
morph build --no-upx           # skip UPX compression
morph build --upx-version 4.2  # pin UPX version
morph build --output bin/      # custom output directory
morph build --entry src/App.mx # override entry file
```
