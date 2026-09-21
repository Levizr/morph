# Windows & Routes

How routes become windows, how windows live and die, and what it costs. API details live in [`Window` / `useWindow`](../api/windows.md); this guide covers the model underneath.

## One file, two uses

A route file is **both a page and a window**. Only files literally named **`route.mx`** are indexed — regular `.mx` components never become routes accidentally:

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

The route id is the **folder path** (relative to `src/`), not the filename. Every folder with a `route.mx` becomes an addressable page; everything else in the folder rides along with it.

- **Navigated to as a page** — rendered inside an existing window: `win.navigate("/auth/login")`
- **Opened as a window** — `new Window("/auth/login", { width, height, data })`

Folders starting with `_` are private (Next.js rule): `src/blog/_components/` is never a route, even if it contains a `route.mx`.

## Route files

A normal `.mx` component with two extras: an optional `windowConfig` export and optional props.

```tsx
// src/auth/login/route.mx
import { morphState, useWindow } from 'morph'
import "./login.css"

export const windowConfig = { title: "Login", width: 400, height: 320 }

export default function LoginPage(props: { userId?: number }) {
  const [error, setError] = morphState("")
  const win = useWindow()   // the window currently rendering this page

  return (
    <div>
      <h1>Sign in</h1>
      <button onClick={() => win.navigate("/settings")}>Cancel</button>
    </div>
  )
}
```

### `windowConfig` is optional

Most routes need no window chrome of their own. Resolution order, most-specific first:

1. Call-site overrides — `new Window("/x", { width: 500 })`
2. The route file's `windowConfig` export
3. The **`[window]` section of `morph.config.json`** (app-wide `width` / `height` / `title`)

A bare `route.mx` opens at the app default size — zero boilerplate per route.

### Props

`data` (on `new Window`) and the second argument of `navigate` arrive as the page's `props`. Scalar props (`number`, `string`, `boolean`) convert to plain C++ values; arrays and objects stay as-is. Missing props read as defaults — never a crash; missing *required* props log loudly at mount.

New props always mount fresh: cached state was built with old props, and silently reusing it would lie.

## Independence: same route, two windows

Opening the same route twice creates two fully independent instances — separate windows, separate trees, separate state:

```tsx
const a = new Window("/settings", { data: { userId: 1 } })
const b = new Window("/settings", { data: { userId: 2 } })
a.navigate("/other")  // only a moves — b stays put
b.close()             // only b dies — a is untouched
```

When two windows share a route, the route alone can't name one of them — that's what explicit `id`s are for. `useWindow("/route")` returns the most-recently-focused live window on that route (the one the user is looking at); use the `id` form when you need precision.

Shared stores (`morphShared`) and events (`morphEvent`) are deliberately **global** — two windows see the same cart, the same bus. Local state (`morphState`) is per window.

## Lifecycle

- A window is created hidden, mounted, then shown. Closing destroys the page, the GL context, and the registration — in that order, safely.
- Navigating swaps the page inside a living window: the old tree and its state are destroyed, the new page mounts fresh. Size, position, and id survive — only content changes.
- The user can close any window at any time (X button, task manager). Handles never dangle: `closed` flips, `on('close')` fires, further calls no-op. See [`Window` / `useWindow`](../api/windows.md#window-lifecycle).

## Memory: thousands of routes cost binary size, not heap

Nothing pre-initializes. Opening the app mounts exactly one route:

| Layer | Cost with 1,000 routes |
|---|---|
| **Manifest** (RID + factory pointer + `windowConfig` per route) | Kilobytes, always resident |
| **Code** (all routes compiled in) | Disk/binary size grows; resident RAM doesn't — the OS demand-pages code on first execution |
| **Mounted trees** (only what's open) | One live page per window — `navigate()` destroys the old root and mounts the new one |
| **Window chrome** (GLFW window + GL context + textures) | The real per-window cost |

### Page cache (planned)

Destroy-on-leave is the default: navigating away frees the page, and coming back remounts fresh. `navigation.cache` (in `morph.config.json`) opts into keep-alive — `0` (default), `N` last pages (LRU), or `"all"`. Cached pages hold tree + state only (no GL chrome), so they're cheap next to open windows.

## Typo safety

Route ids are strings, and strings get typos — so every reference is checked at build time: `new Window("/auth/loign", …)` fails with *"did you mean `/auth/login`?"* plus the list of known routes. The manifest also generates `morph-routes.d.ts` (a `MorphRoute` union at the project root) for editor autocomplete.
