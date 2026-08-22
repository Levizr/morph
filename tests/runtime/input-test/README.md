# Input Test

Runtime test for the `<input>` element: keyboard focus, text editing, caret,
selection, clipboard, and browser-parity length attributes.

## What it shows

- **Controlled input** — `value={name}` + `onChange` (fires per keystroke,
  React semantics); typed text mirrors into the status rows below
- **maxLength={12}** — hard typing/paste cap in UTF-16 code units (HTML spec);
  unset maxLength defaults to the browser hard limit of 524288
- **minLength** — validation-only attribute (never blocks typing, like browsers)
- **placeholder** — dimmed hint text shown while value is empty
- **disabled="true"** — input ignores clicks and never takes focus
- **type="password"** — bullet masking with working caret/selection
- **onFocus / onBlur** — focus state row flips when the field gains/loses focus
- **Editing keys** — arrows, Home/End, Backspace/Delete, Shift+arrows selection,
  Ctrl+A select-all, Ctrl+C/X/V clipboard
- **Mouse** — click to place caret, drag to select, double-click selects word
- **Styling** — background, border, border-radius, color, font-size, cursor: text

## Run

```bash
cd tests/runtime/input-test
morph dev
# or
morph run
```
