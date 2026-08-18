# UI Test

Full-featured landing page UI demonstrating the breadth of Morph's layout and styling capabilities.

## What it shows

- **Navigation bar** — flexbox row with logo and links, `justify-content: space-between`
- **Hero section** — centered column layout, badge, heading, subtitle, CTA buttons
- **Feature cards** — flex-wrap grid of 6 cards with hover border transitions
- **CTA section** — centered call-to-action block
- **Footer** — simple centered text with top border
- **CSS transitions** — `hover` color/border changes with `transition: all 0.3s ease`
- **Inline styles** — colored icons via `style={{ color: "..." }}`

## Run

```bash
cd tests/runtime/ui-test
morph dev
# or
morph run
```
