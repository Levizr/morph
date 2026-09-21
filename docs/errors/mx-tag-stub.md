# mx-tag-stub — Unimplemented Element

**Severity:** warning | **Blocks `morph build`:** no

## What this error means

The tag is registered but not fully implemented in the native runtime yet —
currently `<input>`, `<select>`, and `<textarea>`:

```
warning : mx-tag-stub : Tag <select> is registered but not fully implemented
  hint: Use <div> with custom handling instead of <select>
  Learn more: https://morph.levizr.com/docs/errors/mx-tag-stub
```

This is a **warning**: the build proceeds, rendering the element as a basic
container without full native behavior.

## Why Morph raises it

Morph would rather warn than surprise you. These elements exist in the tag
registry so existing JSX parses, but their native control behavior (caret,
focus, selection, dropdown popups) is still under construction (see
[future text input](../future/text-input.md)). Without the warning you would
stare at a dropdown that never opens and blame your code.

## Example that triggers it

```tsx
// ⚠️ Renders as a plain container, no dropdown behavior — mx-tag-stub
export default function App() {
  return (
    <div>
      <select>
        <option value="a">A</option>
      </select>
    </div>
  );
}
```

## How to fix

For now, build the interaction from supported primitives:

```tsx
// ✅ Custom dropdown from div + state + events
import { morphState } from 'morph';

export default function App() {
  const [open, setOpen] = morphState(false);
  const [choice, setChoice] = morphState("a");

  return (
    <div>
      <button onClick={() => setOpen((o) => !o)}>{choice}</button>
      {open && (
        <div>
          <button onClick={() => { setChoice("a"); setOpen(false); }}>A</button>
          <button onClick={() => { setChoice("b"); setOpen(false); }}>B</button>
        </div>
      )}
    </div>
  );
}
```

For text entry, see the [login example](../examples/login.md), which shows
controlled inputs with validation using currently supported behavior.

Steps:

1. Decide whether you need the native behavior now — if yes, reimplement with
   `div`/`button` + `morphState` as above.
2. If the stub rendering is acceptable (static placeholder), leave it and
   move on; the warning reminds you it is provisional.
3. Track [text input plans](../future/text-input.md) for full support.

## Tuning this rule

Escalate it while stub behavior is unacceptable in your app:

```json
{
  "lint": {
    "severities": { "mx-tag-stub": "error" }
  }
}
```

## See also

- [mx-tag](mx-tag.md) — tag does not exist at all
- [Which HTML Elements Morph Supports](../elements/overview.md)
- [Text input plans](../future/text-input.md)

