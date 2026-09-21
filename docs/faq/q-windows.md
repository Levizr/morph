# Windows & Navigation — Questions & Answers

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

> **Scope note:** the API below is shipped and documented for users in [Windows & Routes](../guides/windows-and-routing.md) and [`Window` / `useWindow`](../api/windows.md). This page keeps the *reasoning* behind the decisions. Design source of truth for what remains: [File-Based Windows & Pages](../future/file-routing.md), [Window API](../future/window-api.md), [Multi-Window](../future/multi-window.md).

## How do I open two windows on the same route and control them independently?

This is the core case the whole design is built around. Every `new Window(routeId, …)` creates a **separate instance** — its own native window, its own root layout tree, its own component state. Same route, zero shared state:

```tsx
const a = new Window("/auth/login", { id: "login-a", data: { userId: 1 } })
const b = new Window("/auth/login", { id: "login-b", data: { userId: 42 } })

a.navigate("/settings")   // only a moves — b stays on /auth/login
b.close()                 // only b dies — a is untouched
```

Independence rests on two mechanisms:

1. **Handles are per-instance.** A handle is a view onto one window in the registry, not onto the route. Navigating, closing, hiding, or retitling through `a` can never affect `b`, because the operations resolve to different registry entries.
2. **Explicit `id`s disambiguate.** When two windows share a route, the route alone can't name one of them — that's what `id: "login-a"` is for. `useWindow("login-a")` always means exactly that window (or `null` if it's closed).

The one ambiguous lookup is `useWindow("/auth/login")` — by *route* — with two live windows on it. It returns the **most-recently-focused** live window on that route. Rationale: in practice that call means "the login window the user is looking at", and focus is the best proxy the runtime has. If you need precision rather than intuition, use the explicit `id`.

## Why `<a href>` instead of `<morph-open>` / `<morph-navigate>`?

Early drafts imagined custom attributes — `<button morph-open="settings">`, `<button morph-navigate="main:home">`. They were dropped before ever being built, for four reasons:

1. **You already know `<a>`.** Every developer on earth knows what `<a href="/settings">` does: it navigates. `<button morph-navigate="main:home">` has to be learned, and its `main:home` micro-syntax (`window:page`?) has to be learned on top. Navigation is the most-used action in any app — it should have the lowest learning cost, not a new DSL.
2. **The URL scheme disambiguates for free.** The design needed three behaviors — navigate here, open there, leave the app — and `<a>` encodes all three with zero new attribute names:
   - `<a href="/settings">` — internal route → navigate the current window (same-tab semantics)
   - `<a href="/settings" target="_blank">` — internal route → open as a new window
   - `<a href="https://…">` — external scheme → OS browser, never a Morph window
3. **One checking pipeline, not two.** Internal `href`s are route strings, so the exact same manifest validation (`mx-route-unknown`, typo suggestions) and the exact same RID lowering apply to links, `new Window(...)`, and `navigate(...)`. A parallel `morph-*` attribute family would need its own lint rules, its own lowering, its own tests — for identical semantics.
4. **Nothing to migrate.** The `morph-*` actions were never implemented (an old doc claimed otherwise — that claim was wrong and has been corrected), so choosing `<a>` costs nobody a rewrite.

What `<a>` does *not* do: render a web page. Morph has no browser engine — an `<a>` never embeds the web inside your app. External URLs escape to the OS browser (`xdg-open` / `open` / `ShellExecute`).

## Why an independent GL context per window instead of one shared context?

Each window owns its own OpenGL context today (`glfwCreateWindow(…, nullptr, nullptr)` — the last `nullptr` means "share with nothing"), and the plan keeps it that way. Sharing sounds efficient — upload the font atlas and textures once instead of N times — but the savings are small and the costs are real:

| | Independent (chosen) | Shared |
|---|---|---|
| GPU memory | Atlas/textures uploaded once **per window** | Uploaded once, visible everywhere |
| Per-frame cost | `MakeContextCurrent` switch per window | **Same switch** — sharing contexts doesn't remove it; every window still renders into its own framebuffer |
| Failure isolation | A GL bug in one window can't corrupt another's resources | One bad texture delete can blank every window |
| Lifetime discipline | Close a window, free its context, done | Closing a window must not free resources another still uses — every shared object needs reference counting across windows |
| Implementation | Status quo, zero new code | New ownership protocol in the compositor |

So sharing's *only* win is duplicate texture memory, bought with cross-window lifetime hazards and a new ownership protocol — while the per-frame context switch people usually hope to eliminate exists either way. If memory ever matters, the plan shares just the font atlas through the existing CPU-side glyph cache, not the GL context. Full isolation is the default that survives contact with real apps (and with users who close windows via the task manager mid-frame).

## Why integers (WID/RID) instead of plain string ids?

Strings are for humans; integers are for machines. You write `"login-a"` and `"/settings"` — but what *runs* is `useWindow_WID(3)` and `create_RID(app::routes::kSettings)`, switch-dispatched with zero hash lookups. Three reasons:

1. **It's the same trick as everything else.** State tags lowered to `MID_*` integers, style keywords to `CSS::` enums, node types to `NodeType` — window and route ids were the last per-call string lookups in the system. Integers finish the job the kill-runtime-strings project started.
2. **Typos die twice.** The manifest check (`mx-route-unknown`, "did you mean `/auth/login`?") catches `"/auth/loign"` in `.mx` files — and independently, `app::routes::kAuthLoign` *doesn't exist*, so the generated C++ fails compilation even if the linter is skipped. Strings give you one net; integers give you two.
3. **Dynamic ids still work.** Only *literal* ids intern. `useWindow(someVar)` falls back to a runtime string lookup with an `mx-window-dynamic` warning — the escape hatch stays, it just stops being the default.

## Why two integers — RID *and* WID? Why not one id for everything?

Because they answer different questions. **RID** (*what* to show) is a compile-time constant — one per `route.mx` file, baked into `app::routes::`. **WID** (*which instance*) is a runtime handle — N per route, minted by each `new Window(...)`. `create(RID) → WID`; `navigate(WID, RID)`.

One id space can't express "two windows, same route": the moment two instances share an identifier, every operation on it is ambiguous. Splitting them makes the independence structural — `a.navigate(...)` names instance `a` showing route X, and there is no spelling under which it could touch instance `b`.

## Why does C++ get window control but no bare `useWindow()`?

`native.cpp` gets `app::windows::{open, navigate, close, on_close, …}` taking RID in and WID out, because native-driven flows are real: a tray icon that reopens the main window, a global hotkey that pops settings, a C++ downloader that opens a progress window on completion. Forcing those through a JS round-trip would be pure ceremony.

But there is deliberately no C++ `useWindow()` *with no argument*. In JSX, "the current window" is compile-time context — the compiler knows which window's tree it is emitting code for. A C++ function has no such context; "current" would mean "whichever window happened to invoke this", which is ambient state smuggled across the FFI — exactly the kind of spooky behavior the interop layer refuses to provide. C++ always names its WID explicitly. If native code needs the invoker's window, JS passes the WID in as an argument. Explicit at the boundary, convenient inside each language.

## Why no back/forward history in `navigate()`?

Direct swap only — `win.navigate("/settings")` replaces the page, no stack, no back button. Two reasons: nothing in the planned validation apps needs history (desktop windows aren't browser tabs; users expect dialogs and tool windows to open and close, not to go "back"), and history is purely additive — it can be layered on later without changing a single existing call. The plan says *no* to speculative machinery, not to the feature forever. If your app would genuinely use per-window history, that's feedback worth sending to `suggestions.morph@levizr.com` — it's the same address the dynamic-segments debate is waiting on.

## The user closed my window with the X button — why didn't my handle crash?

Because handles are views, not owners. The `WindowManager` registry owns every window (`shared_ptr`); your handle resolves its WID through the registry **at every call**. User-initiated close (X button, task manager, OS shutdown) routes through the GLFW close callback, which erases the registry entry, marks the handle `closed`, and fires `close` on every live handle. After that:

```ts
loginWin.closed            // true — the registry told the truth
loginWin.close()           // safe no-op, returns false — never crashes
loginWin.navigate("/x")    // returns false instead of dereferencing a dead window
```

The design assumption is blunt: **every window can die at any moment, and every operation must survive that.** Reopen with `new Window(...)`; old handles stay `closed` forever. This is also why closing is the one operation that's already implemented in the runtime while `open()` and `navigate()` are still stubs — destruction safety came first.

## I have thousands of routes — do they all load into memory at startup?

No. Nothing pre-initializes. Opening the app mounts exactly **one** route; the other 999 cost you binary size, not heap. The full breakdown:

- **Manifest (~100 bytes/route, always resident).** At startup the runtime holds a table of RID → factory pointer + `windowConfig`. Kilobytes for thousands of routes.
- **Code (disk, not RAM).** Every route compiles into the one binary, so the executable grows — but machine code lives in the text segment, which the OS demand-pages in on first execution. Unvisited routes occupy disk sectors, not resident memory.
- **Mounted trees (only what's open).** A route costs real heap only while mounted: its node tree + signals + layout caches. `new Window(RID)` factory-builds the tree on demand; `navigate()` destroys the old root and mounts the new one; `close()` frees the tree, the GL context, and the window chrome together.
- **Window chrome (the actual per-window cost).** GLFW window + GL context + font atlas/textures dwarf everything above — which is why "same route, two windows" costs 2× chrome while sharing nothing else.

Two consequences worth knowing: first, Morph is AOT with no dynamic route splitting planned — if your route count ever makes the binary itself a problem, compression (already supported in the build) is the lever, not lazy loading, because loading was never eager. Second, navigation **resets state by default** (see the next question) — cheap memory and fresh mounts are the same decision.

## Navigating away destroys the page's state — can I keep it?

By default, yes it destroys: leave a page, its tree and `morphState` are freed; coming back remounts fresh with re-initialized state and lost scroll position. That's the memory-minimal default, and it falls out of the direct-swap `navigate()` — there is no history stack keeping dead pages alive.

If your app wants web-like instant revisits, opt into the keep-alive cache in `morph.config.json` — it's your decision, per app:

```json
{ "navigation": { "cache": 0 } }
```

`0` (default) is destroy-on-leave. `N` keeps the last N visited pages detached in memory with LRU eviction — re-navigation within the cache is instant with state intact. `"all"` keeps every visited page, for small apps that want everything instant. Cached pages hold tree + state only (window chrome is still freed), so they're cheap next to open windows — and passing new props always forces a fresh mount anyway, since cached state was built with old props.

## Do I need a `windowConfig` export in every `route.mx`?

No — only when the route wants non-default window chrome. Resolution is a three-level fallback, most-specific first:

1. Call-site overrides: `new Window("/settings", { width: 500 })` or `<a href="/settings" target="_blank" width={500}>`
2. The route file's `windowConfig` export — for routes that always want their own size/title (a 400×320 login dialog, say)
3. The **`[window]` section of `morph.config.json`** — the app-wide defaults (`width` / `height` / `title`), which already exist today

A bare `route.mx` with no export and no overrides opens at the app default size. The point of the chain: per-route boilerplate drops to zero, while a route that genuinely needs its own chrome (dialogs, splash screens, fixed-size tools) declares it once, co-located with the page, instead of repeating dimensions at every call site.
