# Text Input — Caret, Focus, Selection, Keyboard

**Status:** shipped (core) · **Priority:** high

> **Note:** The core below is shipped, not a proposal — `InputNode` is a real text field. `select` / `textarea` and the `:focus` pseudo-class remain future.

> **Shipped → main docs.** `<input>` behavior (caret, selection, undo/redo, clipboard, attributes) is documented for users in [Elements Overview](../elements/overview.md#input-attributes). This page keeps what remains.

Making `<input>` a real text field. Shipped: `InputNode` (`runtime/cpp/ui/input.h`, ~1000 lines) renders text via the FreeType pipeline with caret, selection, undo/redo, clipboard, and full keyboard editing — key events route to the focused node (`MorphNode::s_focusedNode`, `runtime/cpp/core/window.cpp`).

## Why it matters

The `<input>` element is registered and fully implemented, so users can type into it. `morph check` still flags `select` / `textarea`:

- `morph check` flags them: `registered but not fully implemented in the runtime yet` (`mx-tag-stub`)
- Keyboard events dispatch to the **focused** node (`s_focusedNode`), falling back to the node under the cursor when nothing has focus

## Current state

| Piece | State |
|---|---|
| `InputNode` widget (styled box + scroll + scrollbar) | ✅ Shipped (`runtime/ui/input.h`) |
| `MorphNode::focused` / `s_focusedNode` focus lifecycle + `focus()`/`blur()` | ✅ Shipped (`runtime/cpp/core/node.h`, routed in `window.cpp`) |
| Keyboard routing to focused node + `onKeyEvent`/`onTextChar` consumption | ✅ Shipped (`window.cpp`) |
| Text rendering / caret (blink, measurement-positioned) / selection (click-drag, Shift+arrows, highlight) | ✅ Shipped (`drawTextAndCaret`, `measureTextWidth`) |
| Editing — typing, backspace/delete, arrows, Home/End, Ctrl+A/X/C/V, undo/redo, clipboard, `maxLength`/`password` | ✅ Shipped |
| `EventType::Focus` / `Blur` | ✅ Declared (`event.h`, `node.h`) — **never dispatched** |
| `:focus` CSS pseudo-class | ❌ Not built |
| `select` / `textarea` elements | ❌ Same stub status (`STUB_TAGS` in `morph-parser`) |

## Planned behavior (remaining)

1. ~~**Focus model** — clicking an input sets `focused`; a `focus()`/`blur()` API; `Focus`/`Blur` events dispatched through the existing event system; keyboard events route to the focused node instead of the hovered one~~ ✅ Shipped (events still never dispatched)
2. ~~**Text rendering** — reuse the FreeType text pipeline (`morph_text.h`) for the input value~~ ✅ Shipped
3. ~~**Caret** — blinking cursor at the insertion point, positioned via the same text-measurement used for layout~~ ✅ Shipped
4. ~~**Selection** — click-drag + Shift+arrow selection with a highlight rect~~ ✅ Shipped
5. ~~**Editing** — typing (ASCII + modifiers), backspace/delete, arrows, Home/End, Enter (form semantics), Ctrl+A/X/C/V~~ ✅ Shipped (undo/redo + clipboard included)
6. **`select` / `textarea`** — once the focus + text plumbing exists, these become small variations (single-line vs multi-line wrap)

## Open questions

- **Keymap** — standard Linux/Windows editing shortcuts only, or configurable?
- **Composition / IME** — CJK input needs an IME hook; out of scope initially?
- **Focus ring** — CSS `:focus` pseudo-class is a natural pairing (currently unsupported)
- **Clipboard** — X11 clipboard integration for Ctrl+C/V

## Build steps (remaining)

1. ~~Focus model: `focused` lifecycle, Focus/Blur events, key routing to focused node~~ ✅ (except event dispatch)
2. ~~Text + caret rendering in `InputNode`~~ ✅
3. ~~Selection + editing shortcuts~~ ✅
4. `:focus` CSS pseudo-class + Focus/Blur event dispatch
5. `textarea` (multi-line) + `select` (dropdown)