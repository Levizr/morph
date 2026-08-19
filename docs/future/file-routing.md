# File-Based Windows & Pages (`route.mx`)

**Status:** future · **Priority:** high · **Depends on:** [Window API](window-api.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

A route file is **both a page and a window**:

- **Navigated to as a page** — rendered inside the current window (`win.navigate("/auth/login")`)
- **Opened as a separate window** — `new Window("/auth/login", { width, height, data })`

## The convention — `route.mx`

Inspired by the Next.js App Router — where only files named `page.js` become routes ([routing docs](https://nextjs.org/docs/app/building-your-application/routing), [pages and layouts](https://nextjs.org/docs/app/building-your-application/routing/pages-and-layouts)) — Morph indexes only files named **`route.mx`**.

The **route id is the folder path** (relative to the source root `src/`), not the filename:

```
src/
├── auth/
│   ├── login/
│   │   ├── route.mx          →  /auth/login
│   │   ├── login.css         ←  co-located styles
│   │   └── login_helpers.cpp ←  co-located C++
│   └── register/
│       └── route.mx          →  /auth/register
├── settings/
│   └── route.mx              →  /settings
└── components/               ←  NOT routes (no route.mx)
    └── Button.mx
```

Every folder becomes a URL-like route; everything else in the folder is co-located with it. Regular `.mx` components stay components — a file is only indexed if it's literally named `route.mx`.

## The route file

A normal `.mx` component. The `windowConfig` export is **optional** — it provides defaults for when the file is opened as a window (overridable at construction time):

```tsx
// src/auth/login/route.mx
import { CSS, morphState } from 'morph'

CSS.load("./login.css")

export const windowConfig: WindowConfig = { title: "Login", width: 400, height: 320 }

export default function LoginPage(props: { userId?: number }) {
  const [error, setError] = morphState("")
  const win = useWindow()            // the window currently rendering this page

  return (
    <div>
      <h1>Sign in</h1>
      {props.userId && <p>Welcome back, user {props.userId}</p>}
      <button onClick={() => win.navigate("/settings")}>Cancel</button>
    </div>
  )
}
```

## Opening a route as a window

```ts
// any route can become a window
const a = new Window("/auth/login", {
  width: 400,          // overrides windowConfig.width
  height: 320,
  title: "Sign in",
  id: "login-window",  // optional — used by useWindow("login-window")
  data: { userId: 42 } // passed to the page as props
})
a.show()
```

- The second argument overrides `windowConfig` per-property; anything unspecified falls back to the file's `windowConfig`
- `data` is delivered to the page as its `props` — identical to how navigation passes props
- Without a `windowConfig` in the route file, `new Window(routeId, ...)` must supply `width`/`height`

## The `useWindow` hook

Reactive access to a window handle from inside any component:

```ts
const win = useWindow()              // current window — the one rendering this component
const win = useWindow("login-window")  // any window, by the id given at creation
const win = useWindow("/auth/login")   // or by route id (opens if not running)
```

The returned handle exposes window operations (the exact API is free-form — this is the shape):

```ts
win.navigate("/settings", { theme: "dark" })   // swap this window's page
win.close()                                    // close the window
win.show() / win.hide()                        // visibility
win.title = "New Title"                        // live title updates
win.on('close', () => { ... })                 // lifecycle events
```

- `useWindow()` — no argument. Every component is compiled into a specific window's tree, so the compiler resolves the containing window statically (the same way React hooks get context without an argument). No ids to manage in the common case.
- `useWindow("id")` — for reaching windows created elsewhere (e.g. a tray that controls the main window)
- As a hook it can be reactive — a component calling `win.on('close')` could re-render when the window closes

## How it will work

1. **Compiler pass** — at build time, scan the project for files named `route.mx` and add each to the **manifest** (`folder path → component + windowConfig`). Nothing else is indexed — components never become routes accidentally
2. **`new Window(routeId, config)`** — resolved through the manifest at runtime: construct the window from the route's `windowConfig`, apply the constructor overrides, mount the component with `data` as props
3. **`win.navigate(routeId, props)`** — mounts the target route's component into the window's existing root layout tree (reusing the node-tree swap machinery hot reload already uses), passing `props`
4. **`useWindow()`** — compile-time: the component's containing window id is threaded through the IR; runtime: a registry lookup returns the live handle

### Runtime bridge requirements

- Imperative and file-based windows are the same object — `new Window(routeId)` and a manifest-resolved window are indistinguishable
- Opening the same route twice creates two independent windows (each gets its own state)
- Navigating within a window preserves the window's identity (size, position, id) — only the page changes

## Current state

| Building block | State |
|---|---|
| `windowConfig` export parsing (`jsx_walker.py`) | ✅ Shipped |
| Multi-window IR (`ir_windows` list in builder) | ✅ Shipped |
| `WindowManager` (register/close/allClosed) | ✅ Shipped |
| Node-tree swap (hot reload) — the navigate primitive | ✅ Shipped |
| `route.mx` scan + manifest generation | ❌ Not built |
| `new Window(routeId, config)` | ❌ Not built |
| `useWindow` hook | ❌ Not built |

## Open questions

- **Source root** — route ids are relative to `src/` today; should the base be configurable (`"routes": "app"` in config)?
- **Props typing** — `props` comes from `data`/`navigate` args; types are inferred from the component signature or declared (`interface PageProps`)
- **Hot reload in dev** — adding/removing a `route.mx` updates the manifest live; navigating to a route mid-edit re-mounts it
- **Window ids** — auto (route id) vs explicit (`id` in config); both must be resolvable by `useWindow`
- **Nested folders** — `/auth/login` nests naturally; is there a limit to depth (no — same as Next.js)

## Build steps (when picked up)

1. Manifest pass: scan for `route.mx` files → `folder path → component + windowConfig`
2. `new Window(routeId, config)` via manifest lookup + `data` → props wiring
3. `win.navigate(routeId, props)` via node-tree swap
4. `useWindow()` / `useWindow(id)` compiler + runtime registry
5. Validation app: three routes — opened as windows, navigated between, and one opened both ways at once