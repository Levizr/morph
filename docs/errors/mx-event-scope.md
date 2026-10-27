# mx-event-scope — morphEvent Channel or Subscription Out of Scope

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

`morphEvent` was called inside a component (or another function), or at module
scope without `export`:

```
error : mx-event-scope : `morphEvent` may only appear at module scope
  hint: Move `export const ev = morphEvent(...)` to the top of the file
  Learn more: https://morph.levizr.com/docs/errors/mx-event-scope
```

## Why Morph raises it

An event channel is an app-level message bus endpoint: emitters and
subscribers in different components must resolve to the **same** channel
object. That only works if the channel is created once at module scope and
exported for import. Created inside a component, every render would mint a new
channel — emissions would vanish into channels nobody subscribes to, and
subscriptions would pile up per render. The linter enforces placement so a
"lost event" bug becomes a build error instead. Emitting and subscribing
(`ev.emit(...)`, `ev.on(handler)`) happen inside components — only the channel
*creation* is module-scoped.

## Example that triggers it

```tsx
import { morphEvent } from 'morph';

export default function App() {
  // ❌ New channel every render — mx-event-scope
  const saved = morphEvent();
  return <div>App</div>;
}
```

```tsx
import { morphEvent } from 'morph';

// ❌ Module scope but not exported — mx-event-scope
const saved = morphEvent();

export default function App() {
  return <div>App</div>;
}
```

## How to fix

```tsx
// ✅ Channel at exported module scope; emit/subscribe in components
import { morphEvent } from 'morph';

export const saved = morphEvent<string>();

export default function App() {
  saved.on((msg) => console.log(msg));
  return <button onClick={() => saved.emit("done")}>Save</button>;
}
```

```tsx
// ✅ An export list exports the same channel
import { morphEvent } from 'morph';

const saved = morphEvent<string>();
export { saved };

export default function App() {
  return <button onClick={() => saved.emit("done")}>Save</button>;
}
```

Steps:

1. Hoist `morphEvent<T>()` to the top of the file, add `export`, take no
   arguments (arguments are the removed API — see
   [mx-api-removed](mx-api-removed.md)).
2. Keep `emit` calls where they are; move any `on` call to the top level of
   the component body.
3. Re-run `morph check`.

## Subscriptions must be top-level too

A subscription also needs a component-owned cleanup point. Put
`eventName.on(...)` at the **top level of the component body**, where codegen
can register it and remove it when the window or route unmounts:

```tsx
export default function SettingsPage() {
  // ✅ Lifted into the mount lifecycle; removed on unmount.
  prefsChanged.on((p) => {
    setPickedTheme(p.theme)
  })

  return <body>…</body>
}
```

Nesting it in `morphEffect` or a DOM event handler has no cleanup scope:

```tsx
export default function SettingsPage() {
  morphEffect(() => {
    // ❌ mx-event-scope — listener outlives the page
    prefsChanged.on((p) => { setPickedTheme(p.theme) })
  })
  return (
    <button onClick={() => {
      // ❌ mx-event-scope — every click adds a listener
      prefsChanged.on((p) => { setPickedTheme(p.theme) })
    }}>Subscribe</button>
  )
}
```

The effect is destroyed on unmount, but the channel keeps its listener (and
captured state) alive. A handler subscription is even worse: every click adds
another listener. Use a top-level subscription instead; the handler can read
and write component state normally.

## Tuning this rule

Do not disable this rule. Misplaced channels fail silently at runtime (lost
events); the build error is strictly better.

## See also

- [mx-shared-scope](mx-shared-scope.md) — same placement rule for shared state
- [mx-api-removed](mx-api-removed.md) — old string-key event APIs
- [morphEvent API](../api/morphEvent.md) / [morphEvent FAQ](../api/faq/morphEvent.md)

