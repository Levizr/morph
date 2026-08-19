# CLI Commands

## morph init

Scaffold a new `.mx` project.

```
morph init [name]
```

| Option | Description |
|---|---|
| `--width` | Window width (default: 800) |
| `--height` | Window height (default: 600) |
| `--title` | Window title (default: project name) |
| `--entry` | Entry `.mx` file (default: `src/App.mx`) |
| `-y`, `--yes` | Skip interactive wizard, use defaults |

Without arguments, launches an interactive wizard that asks for project name, window dimensions, title, and Tailwind mode.

## morph dev

Start dev mode with live hot reload.

```
morph dev
```

| Option | Description |
|---|---|
| `--entry` | Override entry `.mx` file |

Builds the dev runtime, connects via IPC, watches source files, and hot-reloads on save.

## morph build

Build an optimized production binary.

```
morph build
```

| Option | Description |
|---|---|
| `--entry` | Override entry `.mx` file |
| `--output` | Output directory (default: `.morph/`) |
| `--static` | Statically link GLFW/FreeType/HarfBuzz (single self-contained file; needs `.a` dev archives) |
| `--upx` | Compress the binary with UPX |
| `--no-upx` | Skip UPX compression |
| `--upx-version` | Pin a specific UPX release (e.g. `4.2.4`); overrides `build.upx_version` in config |

Feature detection is automatic: static linking, symbol stripping, and UPX compression are applied only when all required libraries exist on the system.

## morph check

Lint `.mx` files against the framework's supported surface without compiling. Exit code `0` = clean, `1` = lint errors (or parse failure) — CI-friendly.

```
morph check
```

| Option | Description |
|---|---|
| `--entry` | Override entry `.mx` file |

Checks elements, props, events, inline styles, imported CSS files, imports, `morphState`/`morphEffect` usage, and JS→C++ compatibility. Errors block `morph dev`/`morph build`; warnings are reported only. Tune rules via the `lint` section of `morph.config.json` (see [Configuration](../getting-started/configuration.md)).

## morph run

Build and run in one step.

```
morph run [binary]
```

| Option | Description |
|---|---|
| `binary` | Path to binary (default: `<output>/app`) |
| `--entry` | Override entry `.mx` file |
| `--output` | Output directory (default: `.morph/`) |
| `--static` | Statically link GLFW/FreeType/HarfBuzz |
| `--upx` | Compress the binary with UPX |
| `--no-upx` | Skip UPX compression |
| `--upx-version` | Pin a specific UPX release |

## morph pkg

Package manager for Morph.

```
morph pkg <subcommand> [package]
```

| Subcommand | Description |
|---|---|
| `morph pkg add <package>` | Install a package |
| `morph pkg remove <package>` | Remove a package |
| `morph pkg search <query>` | Search for packages |
| `morph pkg install` | Restore all packages from config |
| `morph pkg list` | List installed packages |

## morph doctor

Check system dependencies.

```
morph doctor
```

| Option | Description |
|---|---|
| `-v`, `--verbose` | Show detailed version info and paths |
| `-y`, `--yes` | Auto-install missing dependencies without prompting |

Checks Python 3.10+, g++ 14+, cmake, make, pkg-config, GLFW, OpenGL, X11, FreeType, HarfBuzz, and optional tools (Node.js, npm, Tailwind).

## morph cache

Clear the fetched CSS cache.

```
morph cache
```

Deletes `.morph/css-cache/`.

## morph translate

Translate a `.ts` or `.js` file to C++.

```
morph translate <file>
```

Converts TypeScript/JavaScript source into equivalent C++ code.
