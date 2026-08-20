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

Creation is **synchronous — like Electron**: `new Window(routeId, config)` returns a valid handle immediately; the window opens fast, and content (route mount + first frame) finishes async. Wait with `ready()` only if your code depends on content:

```ts
// any route can become a window — Electron-style
const a = new Window("/auth/login", {
  width: 400,          // overrides windowConfig.width
  height: 320,
  title: "Sign in",
  id: "login-window",  // optional — used by useWindow("login-window")
  data: { userId: 42 } // passed to the page as props
})
a.show()
await a.ready()        // only when you need the first frame
```

- The second argument overrides `windowConfig` per-property; anything unspecified falls back to the file's `windowConfig`
- `data` is delivered to the page as its `props` — identical to how navigation passes props
- Without a `windowConfig` in the route file, `new Window(routeId, ...)` must supply `width`/`height`

## The `useWindow` hook

Reactive access to a window handle from inside any component. By id/route it is **sync** and returns `null` when the window doesn't exist (it may have been closed by the user — see [Window lifecycle & availability](window-api.md#window-lifecycle--availability)):

```ts
const win = useWindow()                      // current window — sync, always valid
const win = useWindow("login-window")        // by id — null if not running
const win = useWindow("/auth/login")         // or by route id — null if not running
if (!win) return
```

The returned handle exposes window operations (the exact API is free-form — this is the shape):

```ts
win.navigate("/settings", { theme: "dark" })   // swap this window's page (false if closed)
win.close()                                    // close the window (safe no-op if already closed)
win.show() / win.hide()                        // visibility
win.title = "New Title"                        // live title updates
win.closed                                     // true if the user closed it via X / task manager
win.on('close', () => { ... })                 // fires for ANY close, including user-initiated
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

## Typo safety — validated at build time

Route ids and window ids are strings, and strings get typos. The manifest makes every reference **statically checkable** — typos and naming violations die at build time, never at runtime:

```ts
const a = new Window("/auth/loign", {...})   // ✗ typo — caught by morph check before shipping
const w = useWindow("login_widnow")          // ✗ typo + bad id (underscore is allowed; the typo is not)
const v = useWindow("login-window")          // ✓
```

### Generated typed routes (editor-level safety)

The manifest pass also generates `morph-routes.d.ts` — a union of every route and every statically-known window id, shipped into `node_modules/morph`:

```ts
// generated: morph-routes.d.ts
type MorphRoute = "/auth/login" | "/auth/register" | "/settings"
type MorphWindowId = "login-window" | "settings-win"
```

```ts
new Window("/auth/loign", {...})   // ✗ TypeScript error in the editor — autocomplete shows the real routes
useWindow("login-widnow")          // ✗ same
```

Typos die **in the editor** for TS users, and `morph check` catches them **in CI** for everyone else.

### Naming — routes follow Next.js, ids follow their own rules

Route segments adopt the **Next.js App Router folder conventions** — Morph's `route.mx` plays the role of `page.tsx`:

| Next.js rule | Morph | Example | Lint |
|---|---|---|---|
| Segments are **lowercase** (URL-safe; kebab-case recommended) | same | `/auth/login` ✓ — `/Auth/Login` ✗ | `mx-route-case` |
| **Private folders** — `_`-prefixed are not routed | same | `src/blog/_components/` is not a route; referencing it is an error | `mx-route-private` |
| **Route groups** — `(name)` omitted from URL | same (future) | `src/(marketing)/about/route.mx` → `/about` | — |
| **Reserved names** — `page`, `layout`, `loading`, `error`, `not-found`, `template`, `default` | `route`, `layout`, `loading`, `error`, `not-found`, `template`, `default` | can't be a route segment (conflicts with special files) | `mx-route-reserved` |
| URL-safe characters only | no spaces, dots, `\`, `#`, `?`, `&` — allowed: `a-z 0-9 - _ ( ) @` | `/auth/login` ✓ — `/auth login` ✗ | `mx-route-chars` |

> **Dynamic segments (`[param]`, `[...param]`, `[[...param]]`) are NOT planned.** They exist to solve URL routing for web apps — blogs, news sites, docs. Morph renders native windows, not URLs; a route id is a compile-time page identifier, so a segment like `[slug]` has no meaning at runtime. **Not sure this is needed — if you'd build a news feed or article list with Morph, say so:** `suggestions.morph@levizr.com`

Window ids (`id` in `new Window(routeId, { id: "..." })`) are **not paths** — they have their own rules:

- must contain only `a-z 0-9 - _` (no `/`, spaces, dots)
- must not start with a digit, `-`, `_`, or `.`
- must not end with `-` or `_`
- must not be a reserved word (`main`, `root`, `window`, `this`, `self`, `app`)
- must not collide with a route id or another window id
- length cap (default 32)

### Lint rules (`morph check`)

New diagnostics following the existing `mx-*` convention:

| Rule | What it checks | Severity |
|---|---|---|
| `mx-route-unknown` | route string doesn't exist in the manifest (`new Window`, `navigate`, `load`) | error |
| `mx-route-suggestion` | close-match typo: *"unknown route `/auth/loign` — did you mean `/auth/login`?"* | error |
| `mx-route-format` | inconsistent path form — `auth/login` vs `/auth/login` vs trailing `/` | warning |
| `mx-route-case` | uppercase route segment (`/Auth/Login`) | error |
| `mx-route-chars` | forbidden characters in a segment (spaces, dots, `\`, `?`, …) | error |
| `mx-route-reserved` | segment uses a reserved name (`route`, `layout`, `loading`, …) | error |
| `mx-route-private` | referencing a `_`-prefixed (private) folder as a route | error |
| `mx-window-unknown` | id string matches no declared window id (`useWindow("x")`, `morph-open="x"`) | error |
| `mx-window-id-chars` | id contains `/`, spaces, dots, or other forbidden chars | error |
| `mx-window-id-edge` | id starts with a digit / `-` / `_` / `.`, or ends with `-` / `_` | error |
| `mx-window-id-reserved` | id is a reserved word (`main`, `root`, `window`, …) | error |
| `mx-window-id-long` | id exceeds the length cap | warning |
| `mx-window-dynamic` | non-literal id (`useWindow(someVar)`) — can't be verified statically | warning |
| `mx-window-duplicate` | two windows declaring the same explicit `id`, or a window id colliding with a route id | error |
| `mx-route-no-export` | a `route.mx` file without a default export | error |
| `mx-window-attr` | `morph-open` / `morph-close` / `morph-navigate` attributes referencing unknown routes/ids | error |

**How it works:** the check pass collects every route id from the manifest scan + every explicit window id from `new Window(..., { id: "..." })` literals, then cross-references all route/window string literals across `.mx` files. Naming rules run on the declared ids themselves; close matches use edit distance for `mx-route-suggestion`. Rules are configurable in `morph.config` — teams can relax a severity or change the length cap:

```json
{
  "naming": {
    "route":    { "case": "lower", "maxLength": 64, "privateFolders": true },
    "windowId": { "maxLength": 32, "reserved": ["main", "root", "window", "this", "self", "app"] }
  }
}
```

### Layered defense (the typo never ships)

1. **Editor** — typed routes (`morph-routes.d.ts`) → TypeScript error + autocomplete
2. **CI / build** — `morph check` rules above → `morph build` fails fast on errors
3. **Runtime** — the safety net stays: `useWindow(id)` returns `null`, `navigate` returns `false` — a miss degrades gracefully even if dynamic code sneaks one through

## Current state

| Building block | State |
|---|---|
| `windowConfig` export parsing (`jsx_walker.py`) | ✅ Shipped |
| Multi-window IR (`ir_windows` list in builder) | ✅ Shipped |
| `WindowManager` (register/close/allClosed) | ✅ Shipped |
| Node-tree swap (hot reload) — the navigate primitive | ✅ Shipped |
| `morph check` diagnostics framework (`mx-*` codes) | ✅ Shipped — the lint rules plug into this |
| `route.mx` scan + manifest generation | ❌ Not built |
| `new Window(routeId, config)` | ❌ Not built |
| `useWindow` hook | ❌ Not built |
| `morph-routes.d.ts` typed routes | ❌ Not built |
| Route/window naming + reference lint rules (`mx-route-*`, `mx-window-*`) | ❌ Not built |

## Open questions

- **Source root** — route ids are relative to `src/` today; should the base be configurable (`"routes": "app"` in config)?
- **Props typing** — `props` comes from `data`/`navigate` args; types are inferred from the component signature or declared (`interface PageProps`)
- **Hot reload in dev** — adding/removing a `route.mx` updates the manifest live; navigating to a route mid-edit re-mounts it
- **Window ids** — auto (route id) vs explicit (`id` in config); both must be resolvable by `useWindow`
- **Nested folders** — `/auth/login` nests naturally; is there a limit to depth (no — same as Next.js)
- **Dynamic segments** — not planned (native windows ≠ URL routes). Open to feedback if real apps need them: `suggestions.morph@levizr.com`

## Build steps (when picked up)

1. Manifest pass: scan for `route.mx` files → `folder path → component + windowConfig`
2. `new Window(routeId, config)` via manifest lookup + `data` → props wiring
3. `win.navigate(routeId, props)` via node-tree swap
4. `useWindow()` / `useWindow(id)` compiler + runtime registry
5. Lint rules: `mx-route-*` / `mx-window-*` — reference checks against the manifest + Next.js-style naming conventions (`mx-route-case`, `mx-route-reserved`, `mx-route-private`, `mx-window-id-*`) + `morph-routes.d.ts` generation
6. Validation app: three routes — opened as windows, navigated between, and one opened both ways at once (with deliberate typos to prove the linter catches them)