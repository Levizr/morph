# Window Event Listeners (`win.on` / `win.off`)

**Status:** future · **Priority:** medium · **Depends on:** [Window API](api.md), [Multi-Window](multi-window.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Imperative DOM-event listeners on any live window handle — `win.on('keydown', h)` — for everything declarative `on*` props cannot express: observing another window's events, subscribing/unsubscribing dynamically, and fanning one handler out over many windows. Supported events are exactly the events Morph supports today (`click`, `keydown`, `keyup`, `input`, … — see [DOM events](../../api/events.md)); drag events ride the same API when drag support lands.

## Why this exists (and what it is not)

Morph already has two event systems that this does **not** replace:

| System | Job | Why `win.on` is still needed |
|---|---|---|
| `on*` JSX props (`<button onClick>`, `<body onKeyDown>`) | Own-window DOM events, declarative | A prop can only listen to **its own window's** tree and only for its mount lifetime. Dynamic subscribe/unsubscribe and other windows' events have no prop spelling. |
| [`morphEvent`](../../api/morphEvent.md) channels | App-level custom events (`emit` / `on`), cross-window by design | Custom events are **opt-in messages you fire yourself**. `win.on` observes **DOM events a window fires on its own** (its clicks, keys, inputs) — no cooperation needed from the other side. |
| [Desktop Integration](desktop-integration.md) hotkeys | OS-level global shortcuts (fire even when the app is in the background) | Hotkeys summon the app. `win.on` coordinates while the app runs. Different layers, no overlap. |

One sentence: **`morphEvent` is for messages you send, hotkeys are for the OS, `win.on` is for DOM events — any window's, on your schedule.**

## Why not `<body onKeyDown>` (or any prop)

Props fail four ways outside their own mount:

1. **Wrong tree.** `<body onKeyDown>` in main hears main's keys only. There is no prop spelling for "popup's keys" — props attach to nodes, nodes live in one window.
2. **Wrong semantics.** A window-level shortcut manager written as a body prop reads as page content. Reviewers ask "which page is this on?" — the answer ("all of them, none of them") means it never belonged in JSX.
3. **No dynamic attach/detach.** A modal focus-trap must subscribe when the modal opens and release when it closes. Props subscribe at mount and release at unmount — the wrong lifecycle for observing *another* window.
4. **No fan-out.** "Every window's Escape closes its own dialog" needs one registration per live window, including windows opened later. Props cannot enumerate windows; a handle loop can.

## Proposed API

```tsx
import { useWindow } from 'morph'

const popup = useWindow("settings-win")

// Subscribe / release — safe no-ops when the window is closed or missing
popup.on('keydown', handleKeys)
popup.off('keydown', handleKeys)
popup.once('close', handleClose)   // auto-releases after first fire
```

Rules (mirroring the window-handle contract in [Window API](api.md)):

- Handles re-resolve through the registry at call time — `on`/`off` on a closed window no-op instead of crashing, like every other handle operation.
- Listeners die with the observed window (close flushes its listener map) **and** with the observing component (unmount releases what it registered — no leaks across navigations).
- Event objects are the same React-style objects as props (`e.key`, `e.code`, `e.target.id`, …). No `e.window` field — you already know which window you're listening to, because you hold its handle; close over it instead.
- Ordering: window listeners fire **after** the observed window's own node-tree handling for that event (observers observe; they never preempt the observed window's own handlers).
- `e.preventDefault()` ships with this API: cancels the runtime's built-in follow-up for that event (link navigation, button click-synthesis, text insertion). `e.defaultPrevented` reads back the flag.

```tsx
// One handler, many windows — the handle is closed over, not on the event
function ShortcutManager() {
  useEffect(() => {
    const h = (e) => {
      if (e.key === 'Escape') currentDialog.close()
    }
    for (const w of allWindows()) w.on('keydown', h)
    return () => { for (const w of allWindows()) w.off('keydown', h) }
  }, [])
}
```

Explicit non-goals: no capture phase in v1, no `window.addEventListener` global (Morph has no global `window` object — handles are always explicit), no OS-level delivery (that is hotkeys, not this), no element handles (see below).

## Why no `targetElement` (and what to use instead)

Browsers hand you `e.target` as a **live node** — `el.style.color = "red"`, `el.remove()`, `el.click()` all work because the DOM is garbage-collected and the node stays valid as long as you hold it. Morph has no such guarantee: a raw `MorphNode*` dangles the moment its window navigates or unmounts — the exact segfault class the window registry was built to prevent. A live element handle would reintroduce it one layer down.

So `win.on` carries **no element handle**. `e.target` stays what props already give you: `{ id, type }` plus coords. Identity and position travel with the event; behavior stays declarative.

### The alternative: `.map` + per-item state

A canvas of draggable items does not need element handles — it needs per-item identity, and `.map` already gives it:

```tsx
{items.map(it => (
  <div key={it.id}
       onMouseDown={(e) => grab(it.id, e.clientX, e.clientY)}
       style={{ position: "absolute", left: it.x, top: it.y }}>
))}
<body onMouseMove={(e) => drag(e.clientX, e.clientY)}
      onMouseUp={() => drop()} />
```

Each item closes over its own id; positions live in a `morphState` array; the press/move/release cycle is three handlers, zero new API. When drag events land, the same shape keeps working — `onDragStart`/`onDrop` per item, state per item.

**Our commitment:** today this needs two tricks — (1) the move/up handlers must live on `<body>` (or another always-hit ancestor) so a fast drag off the item keeps reporting, and (2) press-and-hold has no pointer capture yet, so a release outside the window can strand a drag. We will land both as small runtime fixes in the existing mouse path — no new API. Until we do, `.map`-drag works for slow/inside drags and frays at the edges.

### What native frameworks do instead of element handles

| Framework | Element access pattern | Lesson |
|---|---|---|
| **Qt** | `itemAt(pos)` returns a live `QGraphicsItem*`, safe via parent ownership + `QPointer` guarded pointers (dangle becomes `nullptr`) | Live handles demand a lifetime system Morph doesn't have. Qt paid for it with parent-tree ownership; Morph's registry-per-handle-kind is the cheaper equivalent, and it stops at windows for v1. |
| **GTK** | No live handles across scopes — you hold a `GtkWidget*` you own a reference on (`g_object_ref`), and events carry the widget pointer only within its own signal emission | Same conclusion from the other direction: even GTK ties the pointer to an owned reference, never a bare cross-scope peek. |
| **Flutter** | No element objects at all — `Listener`/`GestureDetector` widgets wrap a subtree declaratively; hit-testing resolves internally and only callbacks fire | The closest model to Morph's `.map` + closures: identity lives in widget configuration, never in a returned object. |

## Bubbling (browser-like, future)

Today Morph fires **one** handler per event: deepest node first, first handler wins, parents only run when nobody below handled. Browsers fire a **chain**: target → parent → … → root, every level with a handler runs. We will bring the chain — not for parity's sake, for one job the single-fire model cannot do: **composition across component boundaries**.

```tsx
// Parent author never wrote Menu's internals — yet both must run
<div onClick={closeMenu}>
  <Menu />   {/* third-party; items have their own onClick inside */}
</div>
```

`.map` + closures cover delegation inside your own component (see above). Bubbling covers the case where the child handler belongs to someone else's component.

### Two axes, not one

The most misunderstood pair in event design — they are independent:

| Call | Default action (navigate, button-click, text insert) | Parent handlers |
|---|---|---|
| `e.preventDefault()` | ❌ Cancelled | ✅ Still fire |
| `e.stopPropagation()` | ✅ Happens | ❌ Don't fire |
| Both | ❌ Cancelled | ❌ Don't fire |
| Neither | ✅ Happens | ✅ Fire |

Canceling a link navigation must not silence the parent's analytics logger — action and observation are separate channels. `preventDefault()` ships with `win.on`; `stopPropagation()` ships with bubbling. Neither implies the other.

### Cost

Negligible, by construction: the hit-test (the expensive part) runs once; the bubble walks parent pointers (nanoseconds per level); per-level event objects build lazily only where a handler exists. Frames pay zero — dispatch lives on the input path, not the render path. Qt, GTK, browsers, and Flutter all bubble every event at 120Hz+ pointer rates; nobody removed it for performance.

### No `target` yet — we need your ideas

Here is the honest gap: a bubble without a hit-node target composes handlers but cannot do delegation — every level sees itself, never the original child. Browsers solve it with the live node; Qt with `itemAt()` + guarded pointers; Flutter by never returning objects at all. We have ruled out live handles (dangle risk) and `e.target` today names the handler node, not the hit child.

So: **how should a bubbled event name the original target?** An id string (safe, but is `{id, type}` enough)? A snapshot (rect + text + value, frozen at dispatch)? Something we haven't thought of? If you have a better or different idea — especially a real app story where the current `{id, type}` falls short — open an issue or email [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com). This is the one open design question blocking the bubbling page, and we would rather get it right than guess.

## Use cases

- **Modal focus traps.** Parent opens a dialog window, subscribes to its `keydown`, traps `Tab` and closes on `Escape`. The dialog's own code stays trap-free.
- **In-app shortcut manager.** `Ctrl+S` / `Ctrl+P` working in every window without pasting a body prop into every route (see above).
- **Settings sync.** Main observes the settings window's `change` events and applies theme/toggles live — no polling, no shared-store misuse for actions.
- **Multi-document editors.** Each document is a window; a coordinator marks dirty tabs and saves on `Ctrl+S` per window.
- **DevTools observer.** The DevTools window subscribes to the target window's clicks/errors/effects without modifying the target.
- **Toast coordinator.** Any window triggers; one toast window owns display and `Escape`-to-dismiss.
- **Future: drag-drop.** Drop-zone window listens for `dragEnter`/`drop` while the source drags — same API, new event kinds when drag support lands.
- **Palette windows.** Floating tool palettes (`V` select, `B` brush) share shortcuts with the canvas window regardless of focus.

## How native frameworks do cross-window events

| Framework | Mechanism | Notes for Morph |
|---|---|---|
| **Qt** | `QApplication::notify()` intercepts everything; `installEventFilter()` on any widget; `postEvent()` across widgets | Central dispatcher ≈ Morph's `WindowManager`. `win.on` is Morph's `installEventFilter`. |
| **GTK** | Window groups + manual `gtk_propagate_event()` | Same lesson as Qt: groups need an explicit propagation call. Morph's registry walk is the equivalent. |
| **Flutter** | `Listener` widgets per subtree (declarative); global keys via `HardwareKeyboard`/`Shortcuts` widgets bound to focus nodes | Declarative-first like Morph's props; focus-node shortcuts ≈ window-listener fan-out. Confirms the two-layer split (props + focus/shortcut layer). |

Morph's edge: single process, integer handles (no string matching at runtime), registry-owned windows (no dangling like raw Qt pointers). `win.on` should be sync, typed, and safe by construction.

## Current state

| Piece | State |
|---|---|
| `on*` props (own-window, declarative) | ✅ Shipped ([DOM events](../../api/events.md)) |
| `win.on('close', …)` | ✅ Shipped ([Windows](../../api/windows.md)) |
| Window-level key fallback (unhandled keys reach `<body>`) | ✅ Shipped (runtime `KeyCb`) |
| `e.target.{id,type}` + coords on all events | ✅ Shipped |
| `.map` keyed lists + per-item closures | ✅ Shipped (used by budget Ledger) |
| `win.on('keydown' \| 'click' \| 'input' \| …, …)` + `off` / `once` | ❌ Not built (this page) |
| `e.preventDefault()` / `e.defaultPrevented` | ❌ Not built (ships with the above) |
| `allWindows()` enumeration helper | ❌ Not built (with the above) |
| Press pointer-capture (drag robustness) | ❌ Not built (mouse-path fix, no API) |
| Drag event kinds (`dragstart`/`drop`/…) | ❌ Not built (ride `win.on` when they land) |
| Element handles (`targetElement`) | ❌ Deferred — see above; not planned for v1 |
| Bubbling (target → root multi-fire, `stopPropagation`) | ❌ Not built — composition across components needs it; `target` design open (see above) |

## Open questions

- **Full event list at launch** — all supported DOM events from day one, or keys-first? (Keys-first: modals and shortcuts are the proven need; the rest is mechanical.)
- **Observer ordering vs `stopPropagation`** — if the observed window stops propagation, do window listeners still fire? (Proposal: yes — observers are outside the bubble, like Electron's `before-input-event`.)
- **Dev-mode rewire** — hot reload must re-attach `win.on` registrations owned by reloaded components (same class of problem as dev-mode window rewiring in [Window API](api.md)).
- **Capture fix ordering** — land press-capture with `win.on`, or earlier as its own mouse-path fix? (Earlier: `.map`-drag needs it independently.)
- **Bubble target design** — see "No `target` yet" above: id string, frozen snapshot, or something else? Waiting on community input before writing the bubbling page.

## Have a better idea?

This API is unwritten — naming, semantics, and scope are all open. If `win.on` feels wrong, if a use case above is better solved another way, or if you have an event need not listed here: open an issue or email [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com). Concrete app stories beat abstract API debate — "my app needs X because Y" is the most useful feedback you can give.
