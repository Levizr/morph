# Morph Development Plan — API & Architecture

A sequential build plan for the API layer, window system, and file-based routing.

---

## Phase 1: Core Imperative API (Foundation)

**Goal:** Every other feature builds on this. Get this right first.

### 1.1 — Define Module Convention
- [ ] Document the rule: instantiable → `class` + `new`, singleton/utility → static class, no `new`
- [ ] Lock in config-object pattern for all constructors: `new Window({ title, width, height })`
- [ ] Decide final list of Phase-1 modules: `Window`, `CSS`, `App`

### 1.2 — Build `Window` Class
```ts
class Window {
  constructor(config: WindowConfig)
  title: string
  width: number
  height: number
  show(): void
  close(): void
  destroy(): void
  on(event: 'close' | 'resize' | 'focus', handler: Function): void
}
```
- [ ] Wire to your existing OpenGL context creation (one GL context per window)
- [ ] Wire to your layout engine — each `Window` owns one root layout tree
- [ ] Hook into your dirty-rendering system per window (resize = layout dirty + repaint)

### 1.3 — Build `CSS` Static Class
```ts
class CSS {
  static load(path: string): void
}
```
- [ ] Replace/supplement `.css` side-effect imports with explicit API (or support both)
- [ ] Add ambient `.d.ts` for `.css` imports regardless, so raw `import './style.css'` doesn't break TS users

### 1.4 — Build `App` Singleton (lifecycle)
8```ts
class App {
  static quit(): void
  static on(event: 'ready' | 'before-quit', handler: Function): void
}
```

### 1.5 — TypeScript Type Safety
- [ ] Hand-write `.d.ts` for all public classes — never leak compiler-internal types
- [ ] Test that autocomplete/IntelliSense looks clean in VSCode

**Exit criteria:** A user can write a multi-window app entirely with `new Window()`, no file-based system yet.

---

## Phase 2: Verify Imperative API Covers Dynamic Cases

**Goal:** Prove the API handles runtime-created UI (popups, dynamically created windows) before adding sugar on top.

- [ ] Build a test app: login window → button → dynamically creates a settings window
- [ ] Build a test app: in-window popup/modal using your overlay-layer render system
- [ ] Confirm layout dirty propagation works correctly when windows/popups are created/destroyed at runtime
- [ ] Confirm GPU resource cleanup on `window.destroy()` (textures, buffers, GL context)

**Exit criteria:** Dynamic window/popup creation works reliably with no leaks, no layout bugs.

---

## Phase 3: File-Based Window/Page System (Compiler Sugar)

**Goal:** Add Next.js-style convention **on top of** Phase 1/2 — not a replacement.

### 3.1 — Define File Convention
```
windows/
  login.tsx
  settings.tsx
  main.tsx
```
- [ ] Decide config export shape:
```ts
export const windowConfig: WindowConfig = { title: "Login", width: 400 }
export default function LoginWindow() { ... }
```

### 3.2 — Compiler Pass
- [ ] Scan `windows/` directory at build time
- [ ] For each file: read `windowConfig`, generate equivalent `new Window({...})` call
- [ ] Wire each file's default-exported JSX as that window's root content
- [ ] Generate a manifest (window name → entry point) for the runtime to reference

### 3.3 — Runtime Bridge
- [ ] Ensure compiler-generated `Window` instances are indistinguishable from manually-created ones (same class, same API)
- [ ] Confirm imperative `new Window()` still works **inside** a file-based window for dynamic child windows/popups

**Exit criteria:** A user can build an app using only `windows/*.tsx` files with zero manual `new Window()` calls, but can still drop into imperative API when needed (e.g. dynamically opening a window from a button click).

---

## Phase 4: Expand Module Set

Once the pattern is proven with `Window`, repeat for:

- [ ] `Menu` (instantiable — class)
- [ ] `Tray` (usually singleton — static class, revisit if multi-tray needed)
- [ ] `Dialog` (instantiable or static? — likely static methods, no persistent identity: `Dialog.showOpen({...})`)
- [ ] `Notification` (instantiable — class, has lifecycle: show/close)

For each: apply the same checklist —
- [ ] Config-object constructor (if instantiable)
- [ ] Hand-written `.d.ts`
- [ ] Works both imperatively and (if relevant) via file convention

---

## Ongoing: Documentation Discipline

- [ ] Every public module gets one canonical example using **named imports** (`import { Window } from 'morph'`)
- [ ] Document the class-vs-static rule once, link to it from every module's docs page
- [ ] Keep a single `API.md` or docs site section listing all public exports — acts as your own contract checklist when adding new modules

---

## Suggested Order of Attack (Next 2–4 Weeks)

```
Week 1     → Window class + GL context wiring + CSS.load + ambient .d.ts
Week 2     → App singleton + dynamic window/popup test apps (Phase 2 validation)
Week 3     → File-based compiler pass (scan, generate, manifest)
Week 4     → Runtime bridge testing + docs pass + expand to Menu/Tray/Dialog
```

Adjust pace to your actual bandwidth as solo dev — but **don't skip Phase 2**. It's tempting to jump straight to file-based routing since it feels more "finished," but if the imperative core has bugs, the file-based layer just inherits them invisibly.