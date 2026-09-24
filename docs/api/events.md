# DOM events

Handler props (`onClick`, `onDoubleClick`, `onMouseDown`, `onMouseUp`,
`onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onContextMenu`,
`onPointerDown`, `onPointerMove`, `onPointerUp`, `onWheel`, `onScroll`,
`onInput`, `onChange`, `onFocus`, `onBlur`, `onKeyDown`, `onKeyUp`)
receive a React-style event object. Both arrow handlers
(`onClick={(e) => ...}`) and function refs (`onClick={handleClick}`)
work; a ref to a zero-parameter function is called as `f()`, otherwise
as `f(e)`.

```tsx
function onBox(e): void {
  // mouse: e.clientX, e.clientY, e.button, e.buttons, e.detail
  // keyboard: e.key, e.code, e.repeat, e.ctrlKey
  // target: e.target.id, e.target.type, e.target.value (inputs)
}
<div id="pad" onClick={onBox} />
<input id="name" onChange={(e) => setName(e.target.value)} />
```

## Event fields

| Field | Type | Notes |
|---|---|---|
| `type` | string | `click`, `dblclick`, `mousedown`, `mouseup`, `mousemove`, `mouseenter`, `mouseleave`, `contextmenu`, `pointerdown`, `pointermove`, `pointerup`, `keydown`, `keyup`, `scroll`, `focus`, `blur`, `input`, `change` |
| `clientX`, `clientY` | number | cursor position (`x`/`y` alias them) |
| `offsetX`, `offsetY` | number | position relative to the target's box |
| `pageX`, `pageY` | number | position including scrolled-out ancestors |
| `timeStamp` | number | monotonic milliseconds |
| `button` | number | button that changed (0 = left, 1 = middle, 2 = right) |
| `buttons` | number | bitmask of held buttons (1 = left, 2 = right, 4 = middle) |
| `detail` | number | consecutive click count on the same node |
| `key` | string | printable key name (`"a"`, `"control"`, `"Backspace"`, …) |
| `code` | string | physical key (`"KeyA"`, `"Digit1"`, `"Enter"`, …) |
| `repeat` | boolean | true for auto-repeat keydowns |
| `ctrlKey`, `shiftKey`, `altKey`, `metaKey` | boolean | modifier state (true while the modifier itself is held) |
| `deltaX`, `deltaY` | number | wheel delta (`scroll` aliases `deltaY`) |
| `value` | string | input/change only: current field value |
| `target`, `currentTarget` | object | `{ id, type }` (no bubbling: both are the dispatch target; inputs add `value`) |
| `relatedTarget` | object \| null | the other node for enter/leave (`{ id, type }`) |
| `pointerId`, `pointerType`, `isPrimary` | — | `1`, `"mouse"`, `true` (single-mouse engine) |

Clicks fire on release when press and release hit the same node —
press-drag-release elsewhere is a drag, not a click. Right-click fires
`contextmenu` (no focus change). Pointer handlers fire alongside the
matching mouse handlers, like browsers. `onWheel` and `onScroll` both
receive wheel scrolls.
