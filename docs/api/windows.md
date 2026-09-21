# Windows (`Window`, `useWindow`)

Create windows at runtime, control any live window, and move between pages. Handles are lightweight ids — every operation re-resolves through the registry, so a handle can never dangle, even when the user closes the window with the X button.

## Importing

```tsx
import { Window, useWindow } from 'morph'
```

## `new Window(routeId, config?)`

Open any route as a new window. Returns a valid handle immediately (Electron-style); the `id` in `config` makes it addressable by name.

```tsx
const settings = new Window("/settings", {
  width: 500,          // overrides the route's windowConfig
  height: 400,
  title: "Settings",   // overrides the route's windowConfig
  id: "settings-win",  // optional — for useWindow("settings-win")
  data: { userId: 42 } // passed to the page as props
})
settings.show()
```

| Field | Type | Description |
|-------|------|-------------|
| `routeId` | `string` | Route id — the `route.mx` folder path (`/settings`, `/auth/login`). Typos fail the build (`mx-route-unknown`, with a did-you-mean suggestion). |
| `config.width` / `config.height` | `number` | Optional overrides. Resolution: overrides → the route's `windowConfig` export → the app-wide `[window]` defaults in `morph.config.json`. |
| `config.title` | `string` | Same fallback chain as size. |
| `config.id` | `string` | Optional explicit id (`a-z 0-9 - _`, must not start with a digit). Addressable via `useWindow(id)`. |
| `config.data` | `object` | Delivered to the page component as `props`. |

Opening the same route twice creates two fully independent windows — separate state, separate trees. Close one and the other is untouched.

```tsx
const a = new Window("/auth/login", { id: "login-a", data: { userId: 1 } })
const b = new Window("/auth/login", { id: "login-b", data: { userId: 42 } })
```

## `useWindow()`

Access a window handle from inside any component:

```tsx
const win = useWindow()               // the window rendering this component — always valid
const win = useWindow("settings-win") // any window, by id — see missing-window rule below
const win = useWindow("/auth/login")  // or by route id — the focused live window on that route
```

**Rule:** `useWindow()` with no argument works directly in the component body and in event handlers. It does **not** work inside module-level helper functions — those have no window context (same reason React hooks have call-site rules). Pass the handle in instead:

```tsx
// ✗ module scope has no window — hard build error
function retitle() { const w = useWindow(); w.title = "x" }

// ✓ pass the handle from event context
function retitle(w) { w.title = "x" }
<button onClick={() => retitle(useWindow())}>retitle</button>
```

### Missing windows

`useWindow(id-or-route)` for a window that doesn't exist (never opened, or closed by the user) yields an invalid handle — it is **not** `null`, so test it with `closed`, not truthiness:

```tsx
const win = useWindow("login-window")
if (win.closed) return   // missing or closed — every operation below degrades safely
```

## Handle API

| Operation | Description |
|---|---|
| `win.navigate("/settings", { theme: "dark" })` | Swap this window's page. Returns `false` if closed. The old page's state is destroyed (fresh mount); the window itself (size, position, id) is untouched. |
| `win.close()` | Destroy the window. Safe no-op if already closed. |
| `win.show()` / `win.hide()` | Visibility without destroying. |
| `win.closed` | `true` once closed — by you **or** by the user (X button, task manager). |
| `win.title` / `win.title = "…"` | Live title read + write. |
| `win.on('close', () => { … })` | Fires for **any** close, including user-initiated. Only `'close'` is supported. |

```tsx
const win = useWindow()
win.navigate("/settings", { theme: "dark" })
win.on('close', () => { console.log("gone") })
```

## Links (`<a href>`)

Markup navigation — no handlers needed. The `href` must be a string literal (dynamic hrefs fail the build; routes are manifest-checked, so typos fail too):

```tsx
<a href="/settings">Settings</a>                                        {/* navigate this window */}
<a href="/settings" target="_blank" width={500} data={{ theme: "dark" }}>Pop out</a>  {/* new window + overrides */}
<a href="https://example.com/help">Help</a>                             {/* external → OS browser */}
```

- Internal `href` navigates the current window (same as `win.navigate(routeId)`).
- `target="_blank"` opens the route as a new window; `width` / `height` / `title` override the route's `windowConfig`, and `data={{…}}` arrives as page `props` (an object expression only — never query strings).
- Any URI scheme (`https:`, `mailto:`, …) opens the OS browser, never a Morph window.
- `<a>` without `href` renders as-is with no click behavior.

## Window lifecycle

Windows are owned by the **registry, not by your handle**. The user can always defeat your bookkeeping — X button, task manager, OS shutdown — so the design assumes **every window can die at any moment, and every operation must survive that**:

- Operations on closed windows return `false` / no-op — never crash.
- `closed` flips to `true` and `on('close')` fires no matter how it died.
- Re-open with `new Window(...)` again; old handles stay `closed`.

## What's next

`Window.ready()`, `resize`/`focus` events, `App.quit()`, and page caching (`navigation.cache`) are planned — see [Q: Windows?](../faq/q-windows.md).
