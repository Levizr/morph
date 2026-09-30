# window-test

Window management + navigation proving ground (build step 1: runtime spine).

- `src/App.mx` — main window (WID 0): two buttons drive the popup via `native.cpp`, status line reflects native-driven state.
- `src/native.cpp` — popup lifecycle through `WindowManager`: hidden creation, `open()`, alias, `onClose`, safe re-open/double-close.

The popup is content-free until the route manifest + mount factories land (build step 3) — it proves hidden state, per-window GL contexts, registry pumping, close safety, and `onClose` for both button-close and user X-button-close.

Verify: `morph build --no-upx`, `<binary> --morph-self-test` (28+ checks), run on `:0` and screenshot both windows.
