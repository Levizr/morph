# Dynamic Styles

Demonstrates runtime-driven class names and inline styles. Theme switching, reactive width, and accent color changes all update the UI without restarting.

## What it shows

- **Template-literal `className`** — `` className={`header ${theme == "light" ? "bg-white" : "bg-gray-900"}`}`` picks Tailwind classes at runtime
- **Inline `style` with state** — `style={{ width: bodyWidth, backgroundColor: accent }}` where values come from `morphState`
- **Tailwind utilities** — `bg-white`, `bg-gray-900`, flex alignment, spacing, `rounded-*`
- **Reactive UI** — buttons toggle theme, resize a panel, and swap accent color

## Run

```bash
cd examples/dynamic
morph dev
# or
morph run
```
