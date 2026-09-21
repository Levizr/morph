# File-Based Windows & Pages (`route.mx`)

**Status:** future · **Priority:** high · **Depends on:** [Window API](window-api.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

> **Decisions update (2026-09-20):** route ids intern to integers (**RID**, `app::routes::` consts — no runtime string lookup), duplicate-route lookup returns the **most-recently-focused** window, `navigate` is a **direct swap** (no history), and markup navigation uses **`<a href>`** (the `morph-*` actions never existed). C++ controls windows via `app::windows::*`. Mounting design (per-instance state) lives in [Route Mounts & Per-Instance State](route-mounts.md). See [Decisions — old vs new](#decisions--old-vs-new). The reasoning behind these calls is answered in detail in [Q: Windows?](../faq/q-windows.md).

> **Shipped → main docs.** The route convention, route files, `new Window`, `useWindow`, `navigate`, RID interning, the memory model, and the `windowConfig` fallback are implemented and documented for users in [Windows & Routes](../guides/windows-and-routing.md) and [`Window` / `useWindow`](../api/windows.md). This page keeps the design history, open questions, and what remains below.

A route file is **both a page and a window** — navigated to in place (`win.navigate("/auth/login")`) or opened separately (`new Window("/auth/login", …)`). Full user docs: [Windows & Routes](../guides/windows-and-routing.md).

## Shipped design (history)

The sections below shipped and moved to main docs — kept here as condensed design history. The manifest scans `route.mx` files (folder path = route id, `_`-private folders skipped), JSX strings lower to `app::routes::` RID consts, window ids lower to WID ints, and the fallback chain is call-site → file `windowConfig` → `[window]` app defaults. Nothing pre-initializes (binary size, not heap); navigation destroys state by default (page cache is future — see below).

## How it works (shipped)

Manifest scan → RID lowering at codegen → mount factories → registry windows. User-facing behavior: [Windows & Routes](../guides/windows-and-routing.md). What remains here: the invariants every future change must preserve.

### Runtime bridge requirements

- Imperative and file-based windows are the same object — `new Window(routeId)` and a manifest-resolved window are indistinguishable
- Opening the same route twice creates two independent windows (each gets its own state)
- Navigating within a window preserves the window's identity (size, position, id) — only the page changes
- **No string lookups at runtime.** Route literals in JSX (`"/settings"`, `href="/auth/login"`) lower to **RID** integer consts; window-id literals lower to **WID** ints (the MID pattern). The manifest owns the string→int tables; strings exist only at build time.
- **By-route lookup is most-recently-focused.** `useWindow("/auth/login")` with two live windows on that route returns the focused one — explicit `id`s remain the precise addressing mechanism.

### RID interning — the namespace trick (same as state)

JSX keeps human-readable strings; generated C++ uses namespace-qualified integer consts (`app::routes::kAuthLogin`), exactly like `app::<ns>::setter` wrappers did for state. Full lowering detail lives in the guide; the invariant that matters here:

Two integers, two jobs: **RID** = *what* to show (compile-time constant, one per `route.mx`); **WID** = *which instance* (runtime handle, N per route). `create(RID) → WID`; `navigate(WID, RID)`. Conflating them would break "same route, two independent windows".

## Navigating with `<a href>` (no `morph-*` tags)

The `morph-open` / `morph-close` / `morph-navigate` attributes from early drafts were **never implemented** — markup navigation uses the browser-familiar `<a>` tag instead. The URL scheme disambiguates, so no new tag name is needed:

```tsx
<a href="/settings">Settings</a>                          {/* navigate current window (same-tab) */}
<a href="/settings" target="_blank">Pop out</a>           {/* open route as a NEW window */}
<a href="/settings" target="_blank" title="…" width={500} height={400}> {/* + overrides */}
<a href="https://example.com/help">Help</a>               {/* external → OS browser, never a Morph window */}
```

- Internal `href`s are manifest-checked (`mx-route-unknown` + suggestion on typos) and lower to RID consts like everything else
- `target="_blank"` without size overrides falls back to the route's `windowConfig`, then to the `[window]` app defaults in `morph.config.json` (same chain as `new Window`)
- External schemes (`https:`, `http:`, `mailto:`…) open via `xdg-open` / `open` / `ShellExecute` — `<a>` never renders a web page inside Morph

## Typo safety — validated at build time

Route ids and window ids are strings in source, and strings get typos. The manifest makes every reference **statically checkable** — typos and naming violations die at build time, never at runtime (and at runtime they aren't strings at all — see [RID interning](#rid-interning--the-namespace-trick-same-as-state)):

```ts
const a = new Window("/auth/loign", {...})   // ✗ typo — caught by morph check before shipping
const w = useWindow("login_widnow")          // ✗ typo + bad id (underscore is allowed; the typo is not)
const v = useWindow("login-window")          // ✓
```

### Generated typed routes (editor-level safety)

The manifest pass also generates `morph-routes.d.ts` — a union of every route and every statically-known window id, written to the project root (picked up by any `*.d.ts`-aware editor):

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

| Rule | What it checks | Severity | State |
|---|---|---|---|
| `mx-route-unknown` | route string doesn't exist in the manifest (`new Window`, `navigate`, `load`) | error | ✅ Shipped (codegen, with suggestion) |
| `mx-route-no-export` | a `route.mx` file without a default export | error | ✅ Shipped (build) |
| `mx-undefined` / `mx-no-morph-import` | names resolve; Morph APIs imported | error | ✅ Shipped (build gate) |
| `mx-route-suggestion` | close-match typo: *"unknown route `/auth/loign` — did you mean `/auth/login`?"* | error | ✅ Shipped (folded into unknown) |
| `mx-route-format` | inconsistent path form — `auth/login` vs `/auth/login` vs trailing `/` | warning | ❌ Not built |
| `mx-route-case` | uppercase route segment (`/Auth/Login`) | error | ❌ Not built |
| `mx-route-chars` | forbidden characters in a segment (spaces, dots, `\`, `?`, …) | error | ❌ Not built |
| `mx-route-reserved` | segment uses a reserved name (`route`, `layout`, `loading`, …) | error | ❌ Not built |
| `mx-route-private` | referencing a `_`-prefixed (private) folder as a route | error | ❌ Not built |
| `mx-window-unknown` | id string matches no declared window id (`useWindow("x")`) | error | ❌ Not built |
| `mx-window-id-chars` | id contains `/`, spaces, dots, or other forbidden chars | error | ❌ Not built |
| `mx-window-id-edge` | id starts with a digit / `-` / `_` / `.`, or ends with `-` / `_` | error | ❌ Not built |
| `mx-window-id-reserved` | id is a reserved word (`main`, `root`, `window`, …) | error | ❌ Not built |
| `mx-window-id-long` | id exceeds the length cap | warning | ❌ Not built |
| `mx-window-dynamic` | non-literal id (`useWindow(someVar)`) — can't be verified statically | warning | ❌ Not built |
| `mx-window-duplicate` | two windows declaring the same explicit `id`, or a window id colliding with a route id | error | ❌ Not built |
| `mx-link-unknown` | `<a href="…">` referencing an unknown route (`href="/auth/loign"`) | error | ❌ Not built |

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
3. **C++ compiler** — `app::routes::setings` doesn't exist → the generated TU fails even if the linter is skipped
4. **Runtime** — the safety net stays: `useWindow(id)` on a missing window yields an invalid handle (test `.closed`), `navigate` returns `false` — a miss degrades gracefully even if dynamic code sneaks one through

## Current state

| Building block | State |
|---|---|
| `windowConfig` export parsing (`jsx_walker.py`) | ✅ Shipped |
| Multi-window IR (`ir_windows` list in builder) | ✅ Shipped |
| `WindowManager` (register/close/allClosed) | ✅ Shipped |
| Node-tree swap (hot reload) — the navigate primitive | ✅ Shipped |
| `morph check` diagnostics framework (`mx-*` codes) | ✅ Shipped — the lint rules plug into this |
| `route.mx` scan + manifest generation | ✅ Shipped (`morph_parser::routes::scan_routes` — sorted RIDs, sanitized consts, `_`-private skip; proven by `route-test`) |
| `navigation.cache` page-cache policy (`0` / N / `"all"`, LRU) | ❌ Not built |
| `[window]` app-default fallback for routes without `windowConfig` | ✅ Shipped for dynamic windows (opts → file `windowConfig` → `[window]` defaults in `__morph_create_window`) |
| `new Window(routeId, config)` | ✅ Shipped (morpher placeholders → RID helpers; proven click-driven in `route-test`) |
| `useWindow` hook | ✅ Shipped (handle = WID int; `useWindow()`/`useWindow(id-or-route)`, methods + `closed`/`title`; module-scope use is a hard error) |
| `win.navigate(routeId, props)` | ✅ Shipped (unmount + clear + remount via helpers; proven click-driven; cache plugs in later) |
| `<a href>` navigation (internal / `_blank` / external) | ❌ Not built — `<a>`/`href` have no handling anywhere today |
| RID/WID interning (`morph_routes.h`, `app::routes::`) | ✅ Shipped (RID consts + runtime WID minting, alias/route lookup, focus order; asserted via `route:*` self-test checks) |
| C++ window API (`app::windows::*`) | ❌ Not built |
| `morph-routes.d.ts` typed routes | ✅ Shipped (project-root file, `MorphRoute` union; `MorphWindowId` follows with `useWindow` ids) |
| Route/window naming + reference lint rules (`mx-route-*`, `mx-window-*`) | ❌ Not built |

## Open questions

- **Source root** — route ids are relative to `src/` today; should the base be configurable (`"routes": "app"` in config)?
- **Props typing** — `props` comes from `data`/`navigate` args; types are inferred from the component signature or declared (`interface PageProps`)
- **Hot reload in dev** — adding/removing a `route.mx` updates the manifest live; navigating to a route mid-edit re-mounts it
- **Window ids** — ~~auto (route id) vs explicit (`id` in config); both must be resolvable by `useWindow`~~ **Decided 2026-09-20:** explicit `id` in config, interned to WID; by-route lookup (`useWindow("/route")`) returns the most-recently-focused live window on that route. Duplicate explicit ids stay a build error (`mx-window-duplicate`).
- **`<a>` prop passing** — ~~query-like `href="/x?y=1"` vs `data={…}` attr — undecided~~ **Decided 2026-09-21:** `data={…}` attr. Query strings imply URL parsing Morph will never need.
- **Nested folders** — `/auth/login` nests naturally; is there a limit to depth (no — same as Next.js)
- **Dynamic segments** — not planned (native windows ≠ URL routes). Open to feedback if real apps need them: `suggestions.morph@levizr.com`

## Decisions — old vs new

| # | Old | New (2026-09-20) | Why |
|---|---|---|---|
| 1 | Route ids are build-time strings resolved through the manifest at runtime | **RID**: JSX strings lower to `app::routes::k*` int consts; manifest owns the table | Zero runtime string lookup (kill-strings ethos); C++ compiler as second typo net |
| 2 | `useWindow("/route")` with duplicates — unspecified | **Most-recently-focused** live window wins; explicit `id`s for precision | Matches user focus intuition; ambiguity resolved without new API |
| 3 | `navigate` history vs swap — open | **Direct swap**, no history | Simpler; history is additive |
| 4 | `morph-open/close/navigate` JSX actions planned | **Never existed; `<a href>` instead** (`target="_blank"` = new window, external scheme = OS browser) | Browser-familiar; scheme disambiguates, no new tag name |
| 5 | Window control JS-only | **C++ API too** (`app::windows::*`, RID in / WID out); C++ always addresses WID explicitly | Native-driven flows (tray, hotkeys); no ambient context across FFI |
| 6 | Window ids auto vs explicit — open | **Explicit `id`**, interned to WID (MID pattern); dynamic ids = runtime fallback + `mx-window-dynamic` | Static verifiability; same rulebook as MID |

## Build steps (when picked up)

1. Manifest pass: scan for `route.mx` files → `folder path → component + windowConfig`, assign RIDs, emit `morph_routes.h` (`app::routes::`) + `morph-routes.d.ts`
2. `new Window(routeId, config)` via manifest lookup (RID at codegen) + `data` → props wiring
3. `win.navigate(routeId, props)` via node-tree swap (direct swap, no history)
4. `useWindow()` / `useWindow(id)` compiler (WID/RID lowering) + runtime registry; most-recently-focused by-route semantics
5. `<a href>` codegen: internal → navigate (RID), `target="_blank"` → new window, external scheme → OS browser
6. C++ `app::windows::*` + `app::routes::` in `morph_api.h`, documented in `native-cpp.md`
7. Lint rules: `mx-route-*` / `mx-window-*` — reference checks against the manifest + Next.js-style naming conventions (`mx-route-case`, `mx-route-reserved`, `mx-route-private`, `mx-window-id-*`) + `morph-routes.d.ts` generation
8. Validation app: three routes — opened as windows, navigated between, and one opened both ways at once (with deliberate typos to prove the linter catches them)