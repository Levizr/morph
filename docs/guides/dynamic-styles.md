# Dynamic Styles

Combine template-literal `className` with `morphState` to create runtime-driven styles that update without restarting.

## Template-Literal Classes

Use JavaScript expressions inside `className`:

```tsx
const [theme, setTheme] = morphState("light")

<div className={`header ${theme == "light" ? "bg-white" : "bg-gray-900"}`}>
  <span style={{ color: theme == "light" ? "#0f172a" : "#f1f5f9" }}>
    Themed header
  </span>
</div>
```

Each branch of the ternary becomes a reactive style effect on the node. When `theme` changes, the className and styles update automatically.

## Reactive Inline Styles

Pass `morphState` values directly in `style` objects:

```tsx
const [width, setWidth] = morphState(200.0)
const [accent, setAccent] = morphState("#6366f1")

<div style={{ width: width, backgroundColor: accent }}>
  Resized by state
</div>

<button onClick={() => setWidth(width + 20)}>Wider</button>
<button onClick={() => setAccent("#22c55e")}>Green</button>
```

## Theme Toggle Pattern

```tsx
const [theme, setTheme] = morphState("light")

<body style={{ backgroundColor: theme === "light" ? "#ffffff" : "#0f172a" }}>
  <div className={theme === "light" ? "bg-white" : "bg-gray-900"}>
    <button onClick={() => setTheme(theme === "light" ? "dark" : "light")}>
      Toggle theme
    </button>
  </div>
</body>
```

## Dynamic Width/Height

```tsx
const [panelWidth, setPanelWidth] = morphState(220.0)

<div style={{ width: panelWidth }}>
  Content
</div>

<button onClick={() => setPanelWidth(panelWidth + 20)}>Wider</button>
<button onClick={() => setPanelWidth(panelWidth > 60 ? panelWidth - 20 : panelWidth)}>
  Narrower
</button>
```

## Conditional Class + Style

Combine both approaches for complex UIs:

```tsx
const [status, setStatus] = morphState("idle")

<div
  className={status === "loading" ? "spinner" : "card"}
  style={{
    borderColor: status === "error" ? "#ef4444" : "#334155",
    opacity: status === "loading" ? 0.6 : 1
  }}
>
  {status === "loading" ? "Loading..." : "Content"}
</div>
```

See the [dynamic example](../../examples/dynamic/README.md) for a full working app.
