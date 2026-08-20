# OS Accessibility — Screen Reader & Keyboard Navigation

**Status:** future · **Priority:** medium

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Native screen readers (NVDA, Windows Narrator, macOS VoiceOver, Linux Orca) don't read pixels — they read an **accessibility tree** that the OS exposes. Windows, macOS, and Linux each have one: UI Automation (UIA), NSAccessibility, and AT-SPI.

The problem Morph must solve: it draws everything itself through OpenGL — there are no native widgets, so **nothing is exposed to the OS by default**. The OS sees a blank canvas. Native toolkits get this for free; custom renderers (Electron embeds Chromium, Qt has `QAccessible`) have to build the tree by hand.

## Why it matters

- **Commercial requirement** — accessibility is legally mandated in many markets (ADA, Section 508, EU Accessibility Act) and required by public-sector buyers
- **Bigger audience** — screen reader users, voice control (macOS Voice Control, Windows Voice Access), and keyboard-only users are real users of real apps
- **Better navigation for everyone** — focus management, Tab order, and keyboard shortcuts improve the app for power users too
- Morph is already composable and semantic (its JSX is HTML-like) — mapping to an accessibility tree is a natural fit

## How it will work

### Accessible tree from the layout tree

Each frame, Morph walks the layout tree and produces a parallel **accessibility tree**, then publishes it to the OS:

| OS | Mechanism |
|---|---|
| Windows | UI Automation (UIA) — `IUIAutomation` provider on the HWND |
| macOS | NSAccessibility protocol / Accessibility API (`AXUIElement`) |
| Linux | AT-SPI over D-Bus |

The layout node → accessibility element mapping mirrors HTML → ARIA:

- **role** — `button`, `text`, `heading`, `link`, `list`, `listitem`, `image`, `navigation`, `dialog`, `tab`, `checkbox`, `radio`, `slider`, `table`…
- **name** — the accessible label (from text content or `aria-label`)
- **value** — current control value (checkbox state, slider position, input text)
- **state** — focused, disabled, checked, expanded, selected
- **focus** — which node has keyboard focus
- **live regions** — announce updates without focus (e.g. a toast notification)

### API surface

ARIA-style attributes on any element, resolved at compile time:

```tsx
<button role="button" aria-label="Close" onClick={close}>×</button>

<div role="navigation" aria-label="Main">
  <a href="#" tabindex={0}>Home</a>
</div>

<input type="text" placeholder="Search" aria-label="Search documents" />

<div aria-live="polite">{toastMessage}</div>   {/* announced without stealing focus */}
<div aria-hidden={true}>decorative divider</div>
```

```tsx
const win = useWindow()
win.announce("Invoice saved")   // push a live-region announcement
win.focus()                     // move focus to the window
win.focusNext() / win.focusPrev()
```

### Focus & keyboard navigation

- **Tab order** — derived from document order (or explicit `tabindex`)
- **Visible focus ring** — a clear focus indicator on every focusable element (keyboard users can't see a mouse hover)
- **Full keyboard model** — Tab/Shift+Tab, arrows inside lists/tables/tabs, Enter/Space to activate, Esc to close dialogs
- **Focus trap** — modal dialogs keep focus inside until dismissed
- **Navigation shortcuts** — app-level keybindings (like Ctrl+K, Ctrl+1…9) surfaced as discoverable shortcuts

## Current state

| Piece | State |
|---|---|
| Semantic, HTML-like JSX (the source of roles/names) | ✅ Shipped |
| OpenGL renderer (custom-drawn — nothing exposed to the OS today) | ✅ Shipped — this is what must change |
| Keyboard focus plumbing (partial — `TextInput` work covers caret & focus) | 🚧 Scaffold (see [Text Input](text-input.md)) |
| OS accessibility bridge (UIA / NSAccessibility / AT-SPI) | ❌ Not built |
| `aria-*` attribute parsing + accessible-tree IR | ❌ Not built |

## Open questions

- **Level of effort vs payoff** — UIA is the biggest surface (MSAA for legacy assistive tech?). Start with a minimal but *correct* tree (names, roles, focus) or the full control model (values, live regions, selection)?
- **Custom components** — can user C++ nodes expose custom accessible roles/actions, or is the built-in mapping enough at first?
- **Testing** — how do we CI-test accessibility (OS-provided inspection tools: Windows Inspect, macOS Accessibility Inspector) without a real screen reader?

## Build steps (when picked up)

1. Accessible-tree IR: `layout node → role/name/value/state` mapping in the compiler
2. Focus model: Tab order, focus ring, keyboard activation — reusable by Text Input
3. Windows UIA provider bridge
4. macOS NSAccessibility bridge (needs [Platforms](platform.md))
5. Linux AT-SPI bridge
6. `aria-*` attribute parsing + `win.announce()` / live regions
7. Validation app: a form with headings, buttons, a dialog and a toast — verified against a screen reader