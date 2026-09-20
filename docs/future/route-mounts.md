# Route Mounts & Per-Instance State (3b Design)

**Status:** design · **Priority:** high · **Depends on:** [File-Based Windows & Pages](file-routing.md), manifest (✅ shipped), registry spine (✅ shipped)

> **Note:** This is a design record, not a commitment. It exists so `new Window(RID)`, `navigate()`, and internal `<a href>` get built on answered questions instead of silent guesses. Open questions for the owner are at the bottom.

## Why this needs its own design

Mounting a route is easy for *nodes* and hard for *state*. Today everything is global:

| Subsystem | Today (entry) | Problem for two mounts |
|---|---|---|
| `morphState` | global `__st_*` signals | two windows would share one `count` |
| Effects | global pool, `destroy_all_effects()` | unmounting one window must not kill the other's effects |
| `mid` dispatch | global `setCount(mid,v)` switching over global signals | per-mount signals need per-mount dispatch |
| Shared stores | global `app::<ns>::` | none — global is the *correct* semantics (see below) |
| Events | global `morph::channel` registry | none — app bus stays global |
| Props | entry forbids them; child props bind at compile time | route props arrive as runtime `JsObject` |

The docs promise "opening the same route twice creates two independent windows (**each gets its own state**)". That promise forces per-mount state contexts — a second codegen mode next to the entry path, not a tweak to it.

## Architecture

### One context per mount

Each mount (a route instance in a window) owns a generated context struct holding its signals and its effects:

```cpp
// generated per route.mx (e.g. /auth/login)
namespace app::routes::auth_login {
struct Context {
    morph::Signal<int> count{0};
    morph::Signal<std::string> error{""};
    std::vector<morph::EffectNode*> effects;  // owned, destroyed on unmount
};
std::shared_ptr<Context> mount(MorphWindow* win, WID wid, const JsObject& props);
void unmount(MorphWindow* win, std::shared_ptr<Context> ctx);
}
```

- The entry path is untouched (globals, zero churn, zero risk to existing apps).
- Route node emission reuses `emit_node_with_state` with a different `state_map`: getter `count` → `ctx->count.get()` instead of `__st_count.get()`. Same emitter, different table.
- The manager owns mounts: `map<WID, MountHandle>` where the handle holds the context (type-erased `shared_ptr<void>` with the route's deleter) plus the RID. Windows never see contexts; the registry stays the single owner — same rule as windows themselves.

### Effects: scoped creation + individual destroy

The runtime already has the primitives (`EffectNode::cleanup()`, the `dead` flag skipped by `run_pending_effects`). Two small additions:

1. `morph::destroy_effect(EffectNode*)` — mark dead, unsubscribe, remove from pool/pending, delete. Safe mid-frame (the `dead` check guards the run path; `cleanup()` guards the notify path).
2. Scoped registration — a thread-local "current mount" set by an RAII `MountScope` during `mount()`:
   - Generated route code calls `create_effect_scoped(...)`, which registers into the current scope's `effects` when set, else the global pool (entry behavior unchanged).
   - This also captures **dynamically created effects** (an event handler that calls `create_effect` while its window's scope is… no — handlers run outside mount). Rule: effects created *during* `mount()` are owned; effects created later from handlers are app-global, exactly like today. No spooky ownership, no leaks beyond today's semantics.

Unmount = destroy each owned effect + `delete` the tree (existing) + drop the context (signals die with it).

### `mid` in routes: per-mount dispatch tables (decided 2026-09-20)

`mid` works in routes — no build error. The existing mid grouping (per `(ns, getter)`, `mid → signal` switch) is reused mechanically with signals resolved to context members:

```cpp
namespace app::routes::auth_login {
// same grouping as the global fns, signals are ctx members
void setCount(Context& ctx, uint32_t mid, int v) {
    switch (mid) {
        case 0: ctx.count_hero.set(v); break;
        // ...
    }
}
}
```

`MID_*` index consts stay global and shared (tag→index mapping is deterministic per component definition; dispatch is per-route-namespace so identical indices in two routes never collide). Native `(mount, mid)` access waits with the rest of deferred instance state — generated route code (handlers capturing `ctx`) is the v1 caller.

### Shared stores and events stay global (deliberate)

`shared` means app-level: two login windows see the same cart. That matches the name, needs zero new machinery, and matches how web apps behave across tabs (server state) minus the server. Events (`morph::channel`) are the app bus for the same reason. Both documented as cross-window in the FAQ when 3b lands.

### Route functions/classes: app-global, like modules

A function defined in `route.mx` is a namespaced module binding (`app::routes::auth_login::helper`), callable from anywhere — same as every other module binding. No per-mount duplication: code is stateless, state lives in the context.

Consequence: route module-level functions, classes, and globals **must not reference route state or props** — they emit at namespace scope where no `ctx` exists (a baked `ctx->` there is a build error with a clear message, mirroring "entry components must not declare props"). Helpers take explicit parameters instead. Effects, handlers, and node code reference `ctx` freely — they all emit inside the mount function.

### Props: `JsObject` in, plain C++ out (decided 2026-09-20)

```cpp
std::shared_ptr<Context> mount(MorphWindow* win, WID wid, const JsObject& props);
```

`JsValue` appears **only in the mount prologue** — one extraction per declared prop — and never in context members, node code, or state:

- Scalar props (`number`/`string`/`boolean`) extract to plain C++ members (`int userId;`) via total coercions (`as_int`/`as_string`/`as_bool` — never throw; unconvertible yields zero values; strings are never silently parsed as numbers).
- Composite props (arrays/objects) keep `JsArray`/`JsObject` members — they honestly *are* JS values; no plain-C++ equivalent exists.
- Missing props read as `undefined` → coerce to zero values (never crash); missing *required* props log loudly at mount.
- The mount prologue is the single choke point, so future callers (JS lowering, C++ API) share one conversion rule.

### `useWindow()` lowers to a captured `__wid`

The mount function receives the mounting window's WID; generated code binds it as `__wid`, and `useWindow()` (no arg) lowers to that variable. Entry windows bind `__wid` as their constant WID. One rule, both paths — no ambient context crosses any boundary.

```cpp
// inside mount(auth_login): generated
const WID __wid = wid;   // useWindow() → handle for THIS window
```

### Factories: N+1 IR builds

`build.rs` already builds the entry graph. For each manifest route, it builds a **per-route module graph rooted at that `route.mx`** (shared components compile into each mount function that uses them — binary size grows per route, same as today's per-project codegen, no sharing tricks in v1), then emits `mount`/`unmount` into the app TU. The manifest's `windowConfig` feeds window creation; `data` feeds props.

### Unmount, navigate, cache

- `unmount`: destroy owned effects → `delete` tree → drop context. The window (chrome, GL context, WID) survives — only the page dies.
- `navigate(WID, RID, props)`: unmount current (or detach into cache) → mount new. Window identity (size, position, id) untouched.
- `navigation.cache`: holds `MountHandle`s (tree + context) detached. Cached effects **stay subscribed** — their signals are alive in the held context, so nothing dangles and nothing needs suspend/resume machinery. Memory cost is tree + signals only (chrome is freed), as already decided.
- **Overhead accounting (2026-09-20):** subscribed cache costs **zero on the hot path** — `pump()` iterates live windows only, `run_pending_effects()` runs only enqueued effects, and `create_effect_scoped` is one predictable null check (always null on the entry path). The only cost is pay-per-fire: a cached effect subscribed to a *global* signal (`shared`, event channels) re-runs on every global `set()` — wasted runs scale as cached-pages × global-signal traffic, so heavy shared subscriptions + large caches are the one combination to watch. Effects on purely local signals can never fire while cached (zero cost). Suspend/resume was rejected: it moves cost to transitions, shows stale UI on restore, and would need a branch on every global effect run — taxing the hot path to save the cold path. Default `cache: 0` means nobody pays unless they opt in.

### Native access to route-instance state: deferred

v1 routes are driven via props (in) and events (out). Native `(rid, mount, name)` state access needs instance addressing the C++ API doesn't have yet — follow-up, not v1. `app::windows::*` (3a) covers window-level control; page-internals control waits.

## Build order (when 3b is approved)

1. Runtime: `destroy_effect` + `create_effect_scoped`/`MountScope` + manager `m_mounts` + `MountHandle`.
2. Codegen: route graph builds, `Context` + `mount`/`unmount` emission, route `state_map`, per-mount mid dispatch, `__wid` binding.
3. `new Window(RID, config)` + `navigate()` + internal `<a href>` lowering (all three are one-line RID calls once mounts exist).
4. Props extraction + literal-props lints.
5. Validation app: one route opened twice with different `data` (independent counters — the proof), navigated, cached, unmounted; `mid` inside a route driving per-mount instances.

## Decisions (owner, 2026-09-20)

1. **`mid` in routes: per-mount tables** (not an error) — same grouping, ctx-member signals, global `MID_*` consts shared.
2. **Shared/events global across windows** — matches the names; zero new machinery.
3. **Cached effects stay subscribed** — signals alive in held context; no suspend machinery.
4. **Native route-instance state deferred** — props in, events out for v1.
5. **`__wid` capture for `useWindow()`** — one rule for entry + routes (no objection; proceeding).
