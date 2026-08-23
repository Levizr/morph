# Events

Morph supports mouse and keyboard events on elements via JSX attributes.

## Supported Events

| Event | Fires when |
|---|---|
| `onClick` | Element is clicked (mouse down + up). |
| `onDoubleClick` | Element is double-clicked. |
| `onMouseDown` | Mouse button pressed on element. |
| `onMouseUp` | Mouse button released on element. |
| `onMouseEnter` | Mouse enters element bounds. |
| `onMouseLeave` | Mouse leaves element bounds. |
| `onKeyUp` | Key released while element is focused. |
| `onKeyDown` | Key pressed while element is focused. |
| `onChange` | `<input>` value changed. Fires together with `onInput` on every edit (React-style normalization); handler receives `{ value, type }`. |
| `onInput` | `<input>` value changed — fires on every edit with the current value in `event.value`. |
| `onFocus` | `<input>` gains keyboard focus. |
| `onBlur` | `<input>` loses keyboard focus. |

## Usage

```tsx
<button onClick={() => console.log("clicked!")}>Click me</button>

<div
  onMouseEnter={() => setHovering(true)}
  onMouseLeave={() => setHovering(false)}
>
  Hover me
</div>
```

## Event Handlers with State

```tsx
const [count, setCount] = morphState(0)

<button onClick={() => setCount(count + 1)}>
  Count: {count}
</button>
```

## Event Handlers with Functions

Extract complex logic into named functions:

```tsx
function handleKeyDown(e) {
  if (e.key === "Enter") {
    submitForm()
  }
}

<input onKeyDown={handleKeyDown} />
```

## Hover and Active Styles

CSS `:hover` and `:active` pseudo-classes work natively. Define them in your stylesheet:

```css
.btn:hover {
  background-color: #4f46e5;
}

.btn:active {
  background-color: #312e81;
}
```

The runtime detects mouse enter/leave and applies the hover styles. See [Transitions](../css/transitions.md) for animating hover changes.

## Ancestor Hover Rules

`.parent:hover .child` selectors work — when the parent is hovered, styles apply to the child:

```css
.card:hover .card-title {
  color: #6366f1;
}
```

## Scroll Events

Scroll containers (`overflow: scroll` or `overflow: auto`) handle wheel events automatically. Nested scroll containers work correctly — scroll events propagate to the innermost container under the cursor.
