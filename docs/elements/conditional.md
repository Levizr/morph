# Conditional Rendering

Morph supports conditional rendering using JSX expression syntax.

## AND Operator

Show an element only when a condition is true:

```tsx
const [loading, setLoading] = morphState(0)

{loading === 1 && <div>Loading...</div>}
```

The `<div>` only renders when `loading` is `1`. When false, nothing is rendered.

## Ternary Operator

Show one element or another:

```tsx
const [active, setActive] = morphState(false)

{active ? <div>Active panel</div> : <div>Inactive panel</div>}
```

## Inline Conditions

```tsx
const [count, setCount] = morphState(0)

<div>
  {count > 0 && <span>Positive</span>}
  {count === 0 && <span>Zero</span>}
  {count < 0 && <span>Negative</span>}
</div>
```

## Conditional with Inline Styles

```tsx
const [theme, setTheme] = morphState("light")

<div style={{ color: theme === "light" ? "#000" : "#fff" }}>
  Themed text
</div>
```

## Conditional Classes

```tsx
const [active, setActive] = morphState(false)

<div className={active ? "tab active" : "tab"}>
  Tab content
</div>
```

Or with template literals:

```tsx
<div className={`header ${theme == "light" ? "bg-white" : "bg-gray-900"}`}>
  Dynamic header
</div>
```

## Multiple Conditions

Use a helper function for complex logic:

```tsx
function statusColor(status: string): string {
  if (status === "ok") return "green"
  if (status === "error") return "red"
  return "gray"
}

<div style={{ color: statusColor(status) }}>
  Status: {status}
</div>
```

## Conditional Rendering vs CSS `display: none`

Conditional rendering removes the element from the tree entirely. CSS `display: none` keeps it in the tree but hides it visually. Use conditional rendering when you want to fully remove elements (less memory, fewer nodes to layout). Use `display: none` when you want to toggle visibility quickly without re-creating the element.
