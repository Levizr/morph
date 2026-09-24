# DOM events

Handler props (`onClick`, `onInput`, `onChange`, `onFocus`, `onBlur`,
`onKeyDown`, `onKeyUp`, `onMouseEnter`, `onMouseLeave`, `onMouseDown`,
`onMouseUp`, `onDoubleClick`) receive a React-style event object.
Both arrow handlers (`onClick={(e) => ...}`) and function refs
(`onClick={handleClick}`) work; a ref to a zero-parameter function is
called as `f()`, otherwise as `f(e)`.

```tsx
function onBox(e): void {
  // mouse: e.clientX, e.clientY, e.button
  // keyboard: e.key, e.repeat, e.ctrlKey
  // target: e.target.id, e.target.type, e.target.value (inputs)
}
<div id="pad" onClick={onBox} />
<input id="name" onChange={(e) => setName(e.target.value)} />
```

## Event fields

| Field | Type | Notes |
|---|---|---|
| `type` | string | `click`, `dblclick`, `mousedown`, `mouseup`, `mousemove`, `mouseenter`, `mouseleave`, `keydown`, `keyup`, `scroll`, `focus`, `blur`, `input`, `change` |
| `clientX`, `clientY` | number | cursor position (`x`/`y` alias them) |
| `button` | number | mouse button (0 = left) |
| `key` | string | printable key name (`"a"`, `"Enter"`, `"Backspace"`, …) |
| `repeat` | boolean | true for auto-repeat keydowns |
| `ctrlKey`, `shiftKey`, `altKey`, `metaKey` | boolean | modifier state |
| `deltaY` | number | wheel delta (`scroll` aliases it) |
| `value` | string | input/change only: current field value |
| `target`, `currentTarget` | object | `{ id, type }` (no bubbling: both are the dispatch target; inputs add `value`) |

Clicks fire on release when press and release hit the same node —
press-drag-release elsewhere is a drag, not a click.
