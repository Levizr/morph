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
  constructor(routeId: string, config?: WindowConfig)   // handle is valid immediately (Electron-style)
  title: string
  width: number
  height: number
  id: string | null              // explicit id, if given at creation
  closed: boolean                // true once closed — by you OR by the user (X button, task manager)
  ready: boolean                 // true once content is mounted and the first frame is drawn
  show(): void                   // shows immediately (blank until ready)
  hide(): void
  close(): boolean               // safe no-op if already closed; returns whether it closed
  load(routeId: string, props?: object): Promise<void>  // async content (Electron's loadFile)
  navigate(routeId: string, props?: object): boolean    // swap this window's page; false if closed
  on(event: 'close' | 'resize' | 'focus' | 'ready', handler: Function): void
  ready(): Promise<this>         // resolves when content is mounted (Electron's 'ready-to-show')
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
// Or resolve any window by id/route — sync, null if it doesn't exist:
function useWindow(id: string): Window | null

class App {
  static quit(): void
  static on(event: 'ready' | 'before-quit', handler: Function): void
}

class CSS {
  static load(path: string): void        // already exists today
}
```

### Window creation is synchronous — like Electron

`new Window(routeId, config)` returns a **valid handle immediately**, exactly like Electron's `new BrowserWindow()`:

```ts
// Electron
const win = new BrowserWindow({ width: 800, height: 600 })
win.loadFile('index.html')

// Morph
const a = new Window("/auth/login", {
  width: 400,
  height: 320,          // overrides the route's windowConfig
  data: { userId: 42 }  // delivered to the page as props
})
a.show()
```

Native window creation is fast — the handle exists before any heavy work. What's **asynchronous is content**: mounting the route's component, layout, and the first frame. If your code depends on content, wait for it:

```ts
await a.ready()                 // wait for first frame (or: a.on('ready', ...))
a.navigate("/settings")         // safe anytime after
```

The constructor itself loads the route (like `loadFile`); `win.load(routeId, props)` re-loads a different route into an existing window.

### Window lifecycle & availability

The **registry is the single source of truth** — `WindowManager` owns the window; handles are views. Every operation on a handle re-resolves the window id through the registry at call time, so handles never dangle:

```ts
const loginWin = useWindow("login-window")   // null if it doesn't exist — sync
if (!loginWin) return                         // handle the missing case

await loginWin.ready()                        // wait until content is mounted
loginWin.close()                              // close it ourselves

// …but the user might close it via the X button or the task manager —
// the handle stays safe and reports the truth:
loginWin.closed          // true — updated via registry close events
loginWin.on('close', () => { /* fires for ANY close: user X, task manager, App.quit */ })
loginWin.close()         // safe no-op — returns false, doesn't crash
loginWin.navigate("/settings")  // fails gracefully (returns false)
```

Key rules:

- **User-initiated close** (X button / task manager) routes through the GLFW close callback → `WindowManager` marks the window closed, erases it from the registry, and fires `close` events on every live handle
- **Operations on closed windows never crash** — they return `false` / no-op, because the registry lookup fails instead of dereferencing a dead window
- **Re-opening** — `new Window("/auth/login")` again gives a fresh handle; old handles stay marked `closed`
- **C++ safety** — `WindowManager` holds `shared_ptr<MorphWindow>`; JS handles hold weak references resolved by id. A raw pointer to a deleted window is the segfault this design prevents
- **Typos and bad names never ship** — route ids and window ids are cross-referenced against the manifest at build time, and route segments must follow Next.js naming conventions (`mx-route-*` / `mx-window-*` lint rules + generated typed routes); runtime `null`/`false` behavior is only the last line of defense. See [Typo safety — validated at build time](file-routing.md#typo-safety--validated-at-build-time)

### Module convention (the rule for all future modules)

- Instantiable → `class` + `new` (e.g. `new Window(...)`) — the handle is synchronous, like Electron; async only where content is involved (`load`, `ready`)
- Singleton / utility → static class, no `new` (e.g. `App.quit()`)

Constructors take a config object; `Window` additionally takes a route id for the page it renders.

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