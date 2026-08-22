# Text Input — Caret, Focus, Selection, Keyboard

**Status:** future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Making `<input>` a real text field. Today `InputNode` renders a styled rounded box with full scroll support — it draws **no text, no caret, and handles no keyboard input**.

## Why it matters

The `<input>` element is registered and styled, so users naturally expect to type into it. Right now:

- `morph check` flags it: `<input> is registered but not fully implemented in the runtime yet` (`mx-tag-stub`)
- Keyboard events dispatch to the node **under the mouse cursor**, not to a focused node — there is no focus concept at all

## Current state

| Piece | State |
|---|---|
| `InputNode` widget (styled box + scroll + scrollbar) | ✅ Shipped (`runtime/ui/input.h`) |
| `MorphNode::focused` field | ✅ Declared — **never set or read anywhere** |
| `EventType::Focus` / `Blur` + `"focus"` / `"blur"` strings | ✅ Declared (`event.h`, `node.h`) — **never dispatched** |
| Text rendering / caret / selection | ❌ Not built |
| Keyboard consumption | ❌ `onKeyDown`/`onKeyUp` exist but key events go to the hovered node |
| `select` / `textarea` elements | ❌ Same stub status |

## Planned behavior

1. **Focus model** — clicking an input sets `focused`; a `focus()`/`blur()` API; `Focus`/`Blur` events dispatched through the existing event system; keyboard events route to the focused node instead of the hovered one
2. **Text rendering** — reuse the FreeType text pipeline (`morph_text.h`) for the input value
3. **Caret** — blinking cursor at the insertion point, positioned via the same text-measurement used for layout
4. **Selection** — click-drag + Shift+arrow selection with a highlight rect
5. **Editing** — typing (ASCII + modifiers), backspace/delete, arrows, Home/End, Enter (form semantics), Ctrl+A/X/C/V
6. **`select` / `textarea`** — once the focus + text plumbing exists, these become small variations (single-line vs multi-line wrap)

## Open questions

- **Keymap** — standard Linux/Windows editing shortcuts only, or configurable?
- **Composition / IME** — CJK input needs an IME hook; out of scope initially?
- **Focus ring** — CSS `:focus` pseudo-class is a natural pairing (currently unsupported)
- **Clipboard** — X11 clipboard integration for Ctrl+C/V

## Build steps (when picked up)

1. Focus model: `focused` lifecycle, Focus/Blur events, key routing to focused node
2. Text + caret rendering in `InputNode`
3. Selection + editing shortcuts
4. `:focus` CSS pseudo-class
5. `textarea` (multi-line) + `select` (dropdown)