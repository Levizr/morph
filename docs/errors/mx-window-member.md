# mx-window-member — Unsupported Window Handle Member

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** reported by `morph check` for provable window handles:
> `useWindow(...)`, `new Window(...)`, `Window`-annotated locals and helper
> parameters, and bindings assigned a handle later (`let win; win =
> useWindow()`). Untyped values stay quiet — without an annotation or an
> assignment the compiler has no handle to check.

## What this error means

You access a member on a `Window` handle that Morph does not lower:

```
error : mx-window-member : Unsupported Window member `.focus` — no native lowering
  hint: Window handles support navigate(), close(), show(), hide(), on('close'), title, and closed
  Learn more: https://morph.levizr.com/docs/errors/mx-window-member
```

## Why Morph raises it

A window handle is a `WID` integer that re-resolves through the window
registry on every operation — it is not a C++ object. Only a closed set of
members has a registry call behind it:

| Member | Native lowering |
|---|---|
| `win.navigate(route, props?)` | `__morph_navigate_window(...)` |
| `win.close()` | `WindowManager::get().close(wid)` |
| `win.show()` / `win.hide()` | `WindowManager::get().open/hide(wid)` |
| `win.on('close', fn)` | `WindowManager::get().onClose(wid, fn)` |
| `win.title` / `win.title = "…"` | `WindowManager::get().title/setTitle(...)` |
| `win.closed` | `WindowManager::get().closed(wid)` |

Anything else (`win.focus()`, `win.ready()`, `win["title"]`, a mistyped
member) has no call to emit. Passing it through would print a C++ type error
against `WID` far from the source, so `morph check` rejects it first.

## Example that triggers it

```tsx
import { useWindow } from 'morph';

export default function App() {
  const win = useWindow();
  // ❌ `.focus()` is not part of the handle API — mx-window-member
  win.focus();
  return <div>Hello</div>;
}
```

The same rule applies through helpers — annotate the parameter and the
checker follows the handle:

```tsx
// ❌ reports at `w.focus` — the `Window` annotation opts into handle checking
function focusIt(w: Window) {
  w.focus();
}
```

Late assignments are tracked too, so the check follows the binding rather
than only the declaration:

```tsx
let win;
win = useWindow();
// ❌ reported — `win` is a handle from the assignment onward
win.focus();
```

## How to fix

Use a supported member, or move the operation into native C++:

```tsx
import { useWindow } from 'morph';

export default function App() {
  const win = useWindow();
  // ✅ supported handle member
  win.title = "Focused";
  return <div>{win.title}</div>;
}
```

Steps:

1. Check the member against the table above.
2. Use the supported equivalent (`show()` for visibility, `on('close', …)`
   for lifecycle, `navigate()` for pages).
3. If the operation truly needs native window control, add a `.cpp` helper
   and pass the handle as a `WID` — see
   [native C++](../guides/native-cpp.md).
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Unsupported members have no native code to call;
silencing the diagnostic only moves the failure into the C++ compiler.

## See also

- [`Window` / `useWindow`](../api/windows.md) — the complete handle API
- [mx-route-unknown](mx-route-unknown.md) — unknown `navigate`/`new Window` route ids
- [mx-js-member](mx-js-member.md) — unsupported builtin members
