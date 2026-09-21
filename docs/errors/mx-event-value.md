# mx-event-value — Event Handler Is Not a Function

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

An event prop (`onClick`, `onInput`, ...) is not given a function:

```
error : mx-event-value : Event handler `onClick` on <button> must be a function
  hint: Pass `onClick={() => ...}` or a named function reference
  Learn more: https://morph.levizr.com/docs/errors/mx-event-value
```

## Why Morph raises it

Event handlers compile to native callbacks wired to the node's event
dispatcher. A string, number, or boolean cannot become a callback — and the
most common instance of this error, `onClick={handlePress()}`, *calls* the
function during render instead of passing it, running your handler once at
build and wiring `undefined` as the callback. Morph rejects non-function
values so the mistake surfaces at the JSX site.

## Example that triggers it

```tsx
export default function App() {
  const save = () => console.log("saved");

  return (
    <div>
      {/* ❌ Calls save during render, wires its return value — mx-event-value */}
      <button onClick={save()}>Save</button>
      {/* ❌ A string is not a handler — mx-event-value */}
      <button onClick="save">Save</button>
    </div>
  );
}
```

## How to fix

```tsx
export default function App() {
  const save = () => console.log("saved");

  return (
    <div>
      {/* ✅ Pass the function itself ... */}
      <button onClick={save}>Save</button>
      {/* ✅ ... or wrap it in an arrow when you need arguments */}
      <button onClick={() => save()}>Save</button>
    </div>
  );
}
```

Steps:

1. If you wrote `onClick={fn()}`, remove the trailing `()` — or wrap:
   `onClick={() => fn(args)}`.
2. If you wrote a string/boolean, replace it with a function reference or
   inline arrow.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. A non-function handler can never receive events.

## See also

- [mx-prop](mx-prop.md) — unknown event *name*
- [How to Handle Events](../elements/events.md)

