# Imperative Window / App API

**Status:** future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

A programmatic API for creating and managing windows from JavaScript — `new Window(...)`, the `useWindow` hook, `App.quit()`, `App.on(...)` — layered on top of the existing declarative system. Today windows are declared via the `windowConfig` export or `<morph-window>`; this adds runtime control.

## Why it matters

- **Dynamic UIs** — a button click can spawn a settings window, a modal, or a login flow without recompiling
- **Popups & modals** — overlay-layer windows created and destroyed at runtime
- **Multi-window apps** — the foundational API everything else (file routing, menus, dialogs) builds on
- **Imperative escape hatch** — declarative conventions remain, but anything can be done in code

## Planned API surface

```ts
class Window {
  constructor(routeId: string, config?: WindowConfig)   // e.g. new Window("/auth/login", {...})
  title: string
  width: number
  height: number
  id: string | null              // explicit id, if given at creation
  show(): void
  hide(): void
  close(): void
  navigate(routeId: string, props?: object): void       // swap this window's page
  on(event: 'close' | 'resize' | 'focus', handler: Function): void
}

interface WindowConfig {
  title?: string
  width?: number
  height?: number
  id?: string                    // addressable by useWindow(id)
  data?: object                  // passed to the page component as props
}

// Access the window that rendered the current component — no argument needed,
// the compiler resolves it from the component's compiled window tree:
function useWindow(): Window
// Or reach any window by the id given at creation:
function useWindow(id: string): Window

class App {
  static quit(): void
  static on(event: 'ready' | 'before-quit', handler: Function): void
}

class CSS {
  static load(path: string): void        // already exists today
}
```

### The `Window` constructor

`new Window(routeId)` opens any `route.mx` file as a separate window — the same file that can be navigated to as a page. Route ids are folder paths (like Next.js URLs): `src/auth/login/route.mx` → `/auth/login`. See [File-Based Windows & Pages](file-routing.md) for the full design.

```ts
const a = new Window("/auth/login", {
  width: 400,
  height: 320,          // overrides the route's windowConfig
  data: { userId: 42 }  // delivered to the page as props
})
a.show()
```

### Module convention (the rule for all future modules)

- Instantiable → `class` + `new` (e.g. `new Window(...)`)
- Singleton / utility → static class, no `new` (e.g. `App.quit()`)

Constructors take a config object; `Window` additionally takes a file path for the page it renders.

## How it will work

- **`Window`** — each instance owns one GLFW window + one OpenGL context and one root layout tree. Creation wires into the existing window stack (`WindowManager`), layout engine, and dirty-rendering system. Resize marks layout dirty + repaint, matching today's `MorphWindow` behavior.
- **`App`** — a thin static facade over the runtime lifecycle. `ready` fires after first frame, `before-quit` before teardown (so users can save state).
- **TS types** — hand-written `.d.ts` in the shipped `node_modules/morph` module so autocomplete works; never leak compiler-internal types.

## Compiler story

`new Window(routeId, config)` calls in user JS are translated by `TSToCppTranslator` into `WindowManager` operations, exactly like the existing `morph-*` event actions. The route id is resolved through the `route.mx` manifest (see [File-Based Windows & Pages](file-routing.md)). `useWindow(...)` is resolved at compile time — the component's containing window id is threaded through the IR.

## Current state

| Piece | State |
|---|---|
| `CSS.load()` | ✅ Shipped |
| `windowConfig` export + `<morph-window>` | ✅ Shipped (declarative) |
| `WindowManager` (register/close/allClosed) | ✅ Shipped |
| `Window` / `App` classes, `useWindow` hook | ❌ Not built |
| `.d.ts` for imperative API | ❌ Not built |

## Open questions

- **Overlay layer** — do popups/modal windows share the parent window's GL context (compositor must switch framebuffers) or get their own context?
- **GPU cleanup** — `destroy()` must release textures, buffers, and the GL context; `WindowManager::~WindowManager` currently owns teardown.
- **Dev-mode parity** — `logic.so` hot reload must re-wire imperatively created windows the same way it re-wires declarative ones.

## Build steps (when picked up)

1. `Window` class in C++ wrapping `MorphWindow` + registration with `WindowManager`
2. Manifest lookup for `new Window(routeId, config)` + `WindowConfig` parsing in the TS translator + `.d.ts`
3. `useWindow()` / `useWindow(id)` compiler + runtime registry
4. `App` singleton (quit / ready / before-quit events)
5. Test app: login window → button → dynamically creates a settings window (the Phase-2 validation app from the original design plan)