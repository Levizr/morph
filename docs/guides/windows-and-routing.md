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
- **Linked to in markup** — `<a href="/auth/login">` navigates, `target="_blank"` pops out a window, external URLs open the OS browser (see [`<a href>`](../api/windows.md#links-a-href))

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
3. The **`[window]` section of `morph.config.json`** (app-wide `width` / `height` / `title` / `parent` / `modal` / `role`)

A bare `route.mx` opens at the app default size — zero boilerplate per route. Ownership keys (`parent`, `modal`, `role`) resolve through the same chain: a route file can declare every one of its windows modal, and a single call-site `parent` still wins.

### Props

`data` (on `new Window`) and the second argument of `navigate` arrive as the page's `props`. Scalar props (`number`, `string`, `boolean`) convert to plain C++ values; arrays and objects stay as-is. Missing props read as defaults — never a crash; missing *required* props log loudly at mount.

New props always mount fresh: cached state was built with old props, and silently reusing it would lie.

## Independence: same route, two windows

Opening the same route twice creates two fully independent instances — separate windows, separate trees, separate state:

```tsx
const a = new Window("/auth/login", { data: { userId: 1 } })
const b = new Window("/auth/login", { data: { userId: 42 } })
a.navigate("/other")  // only a moves — b stays put
b.close()             // only b dies — a is untouched
```

When two windows share a route, the route alone can't name one of them — that's what explicit `id`s are for. `useWindow("/route")` returns the most-recently-focused live window on that route (the one the user is looking at); use the `id` form when you need precision.

Shared stores (`morphShared`) and events (`morphEvent`) are deliberately **global** — two windows see the same cart, the same bus. Local state (`morphState`) is per window.

## Ownership: parents, modals, roles

Independence is the default — a window with no `parent` survives every other window's close (browser-style). Ownership is always explicit:

```tsx
const main = useWindow()
const popup = new Window("/settings", { parent: main, modal: true })
```

- **`parent`** (handle, id, route, `"auto"`, or omitted/`null` for independent) links lifetimes: closing an owner destroys its owned subtree first. Declare it anywhere geometry lives — `new Window` opts → route `windowConfig` → `[window]` app defaults.
- **`modal`** (`true`/`false`) blocks every other window's close while the modal lives — X clicks on other windows are swallowed (repeat after it closes), `close()` returns `false`. Modals always close themselves. Requires a resolvable parent.
- **`role`** (`"default"`/`"dialog"`/`"popup"`) is presentation only — decorations and hints, never behavior.
- Owned windows float above their parent; modals additionally center on open and move rigidly with their parent in both directions (a screen-edge-clamped parent stops the follower too — the pair never separates).

## Lifecycle

- A window is created hidden, mounted, then shown. Closing destroys the page, the GL context, and the registration — in that order, safely.
- Navigating swaps the page inside a living window: by default the old tree and its state are destroyed and the new page mounts fresh ([page cache](#page-cache) keeps detached pages for instant restore). Size, position, and id survive — only content changes.
- The user can close any window at any time (X button, task manager). Handles never dangle: `closed` flips, `on('close')` fires, further calls no-op. See [`Window` / `useWindow`](../api/windows.md#window-lifecycle).

## Memory: thousands of routes cost binary size, not heap

Nothing pre-initializes. Opening the app mounts exactly one route:

| Layer | Cost with 1,000 routes |
|---|---|
| **Manifest** (RID + factory pointer + `windowConfig` per route) | Kilobytes, always resident |
| **Code** (all routes compiled in) | Disk/binary size grows; resident RAM doesn't — the OS demand-pages code on first execution |
| **Mounted trees** (only what's open) | One live page per window — `navigate()` destroys the old root and mounts the new one (or detaches it into the [page cache](#page-cache)) |
| **Window chrome** (GLFW window + GL context + textures) | The real per-window cost |

### Navigating: what actually happens

`win.navigate("/settings", { userId: 7 })` swaps the page inside a living window. The window itself (size, position, id, GL context) is untouched — only content changes, in three steps:

1. **Unmount the old page** — its effects are destroyed, its event/channel subscriptions are removed, then its tree is deleted.
2. **Clear the window root.**
3. **Mount the new page** — fresh state seeded from `props`, effects created, tree built and attached.

With the default (`navigation.cache: 0`) that is the whole story: leaving a page destroys it, coming back remounts from scratch. Local state (`morphState`) dies with the page; shared stores (`morphShared`) and events survive because they were never per-page — they're app-global by design.

```tsx
// Settings tab is "advanced"; user navigates away and back:
win.navigate("/auth/login")     // settings page destroyed (cache: 0)
win.navigate("/settings", { userId: 7 })  // fresh mount — tab is "general" again
```

### Page cache

`navigation.cache` (in `morph.config.json`) opts a project into keep-alive. It is read at build time — changing it requires a rebuild:

```json
{ "navigation": { "cache": 2 } }
```

| Value | Behavior |
|---|---|
| `0` (default) | Destroy on leave. No cache, no overhead. |
| `N` | Keep the `N` last pages **per window** (LRU). |
| `"all"` | Unbounded — every left page is kept. |

**What is held.** A cached page is its tree plus its state context — no window chrome (GLFW window, GL context, textures are always freed on navigate). That makes cached pages cheap next to open windows, but they are not free: trees and signals stay in RAM until evicted.

**What restores.** Navigating back hits the cache only when all three match:

1. **Same window** — a window only ever restores its own pages. A settings page detached by a popup is invisible to the main window (and vice versa); closing a window drops its cached pages with effect teardown.
2. **Same route** — `/settings` never restores as `/auth/login`.
3. **Structurally equal props** — `{ userId: 7 }` restores for `{ userId: 7 }`, compared by value (key-by-key, including nested objects), not by how the object was built.

Anything else remounts fresh — and as the [Props](#props) section says, new props always mount fresh, since cached state was built with old props:

```tsx
// cache: 2, popup window:
win.navigate("/auth/login")                  // settings{userId:7} detached (tab "advanced" kept)
win.navigate("/settings", { userId: 7 })     // HIT — tab is still "advanced"
win.navigate("/settings", { userId: 8 })     // MISS — fresh mount for user 8; the user-7 page stays cached
```

**While cached, effects stay subscribed.** A cached page's signals are alive in its held context, so nothing dangles and nothing needs suspend/resume machinery — restore is a reattach, not a replay. The cost to know about: an effect subscribed to a *global* signal (shared store, event channel) still re-runs on every global `set()` while cached. Effects on purely local signals can never fire while their page is detached (zero cost). So large caches combined with heavy global subscriptions are the one combination to watch; `0` (the default) means nobody pays unless they opt in.

**Eviction.** Past the cap, the least-recently-used page is destroyed (effect teardown, then tree delete). Closing a window destroys all of its cached pages immediately — a dead window can never be back-navigated to, so its pages are garbage, not cache.

## Typo safety

Route ids are strings, and strings get typos — so every reference is checked at build time: `new Window("/auth/loign", …)` fails with *"did you mean `/auth/login`?"* plus the list of known routes. The manifest also generates `morph-routes.d.ts` (a `MorphRoute` union at the project root) for editor autocomplete.
