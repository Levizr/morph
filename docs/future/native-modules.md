# Native Modules: `Menu` / `Tray` / `Dialog` / `Notification`

**Status:** future · **Priority:** medium · **Depends on:** [Window API](window-api.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

The first wave of native OS modules after the Window API pattern is proven. Each follows the same convention: config-object constructor (if instantiable), hand-written `.d.ts`, works both imperatively and (where relevant) via file conventions.

## The module set

### `Menu`
Instantiable class — an app menu bar and/or context menus.

```ts
const fileMenu = new Menu({ title: "File", items: [
  { label: "Open", onClick: () => openFile() },
  { label: "Quit", onClick: () => App.quit() }
]})

// context menus attach to elements:
<div onContextMenu={() => fileMenu.showAt(mouseX, mouseY)}>…</div>
```

### `Tray`
Usually a singleton (static class) — system tray icon + menu. Revisit if multi-tray is ever needed.

```ts
Tray.setIcon("./assets/icon.png")
Tray.setMenu([{ label: "Show", onClick: () => mainWin.show() }])
```

### `Dialog`
Static methods, no persistent identity — native open/save dialogs.

```ts
const file = await Dialog.showOpen({ filters: [{ name: "Images", exts: ["png", "jpg"] }] })
```

### `Notification`
Instantiable class with a lifecycle (show/close) — OS notifications.

```ts
const n = new Notification({ title: "Build finished", body: "morph run succeeded" })
n.show()
```

## How it will work

- **Config-object constructors** everywhere (the project-wide rule: instantiable → `class` + `new` + config object)
- Each module compiles to a small C++ class wired into the runtime; JS calls become direct C++ calls (no IPC — same process)
- `.d.ts` shipped in `node_modules/morph` for full editor autocomplete
- One canonical example per module using **named imports** (`import { Menu } from 'morph'`)
- A single `API.md` (or docs section) lists all public exports — the contract checklist when adding modules

## Current state

| Module | State |
|---|---|
| `Menu` / `Tray` / `Dialog` / `Notification` | ❌ Nothing in the runtime (grep confirms no widget/class exists) |
| `morph check` stub tags | ⚠️ `input` / `select` / `textarea` flagged as registered-but-unimplemented; native modules aren't registered at all |

## Open questions

- **GLFW-only now** — Menu/Tray/Dialog need OS APIs (GTK/Qt-style native dialogs, X11 tray). Morph is GLFW/EGL-based; native module depth depends on platform abstraction work (see [Platforms](platform.md)).
- **Async dialogs** — `Dialog.showOpen` returning a `Promise` fits the existing coroutine scheduler (`morph::Result<T>`), but blocking native dialogs run on which thread?
- **Ordering** — build `Dialog` first (most self-contained, no persistent state) or `Menu` (most visible value)?

## Build steps (when picked up)

1. `Dialog.showOpen`/`showSave` — static class, async via coroutines
2. `Menu` — app menu bar wired to `MorphWindow`
3. `Notification` — class with show/close lifecycle
4. `Tray` — singleton; revisit only if multi-tray becomes real