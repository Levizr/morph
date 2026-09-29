# Desktop Integration — Hotkeys, Deep Links & OS Hooks

**Status:** future · **Priority:** high · **Depends on:** [Window API](api.md), [Platforms](../shipped/platform.md) (shipped)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Everything that makes an app feel like it *belongs* on the desktop instead of visiting from the web. Today these flows are reachable only by dropping into `native.cpp` — hand-rolled, untyped, and invisible to `morph check`. The plan: first-class JS APIs with the same registry safety as windows (every operation survives the user doing something unreasonable, like quitting mid-handshake).

## Why it matters

- **Power users live on hotkeys** — a global `Ctrl+Shift+Space` that summons your app from any workspace is the difference between "installed" and "used daily"
- **Deep links make apps addressable** — `myapp://invoice/42` from an email should open invoice 42, not a second copy of your app staring blankly
- **Second launch must not mean second app** — users *will* double-click the icon twice; the OS owes them one window, not two processes fighting over a config file
- **Drag-drop is the desktop's native API** — half of all "import" flows in real apps start with a file landing on a window

## The module set

### Global hotkeys

Window-scoped today (key events route to the focused node); *global* hotkeys fire even when the app is in the background:

```tsx
import { Hotkey } from 'morph'

const summon = new Hotkey("ctrl+shift+space", () => {
  mainWin.show()
  mainWin.focus()
})
summon.register()     // OS-level grab; throws a typed error on conflict
summon.unregister()   // auto-cleaned when the owning window closes
```

Conflict with another app's hotkey is a *result*, not a crash — the API returns which registration won, because two apps fighting over `Ctrl+Shift+Q` is a tale as old as time.

### Deep links / protocol handlers

```tsx
// morph.config.json
{ "protocol": "myapp" }   // registers myapp:// with the OS at install

// any route.mx — second launch arrives as props, not a second process
export default function InvoicePage(props: { invoiceId?: string }) { … }
```

First launch opens normally; every later `myapp://…` invocation forwards its URL to the running instance, which routes it like a navigate. (Yes, this is just `<a href>` with extra steps — the OS is the link now.)

### Single-instance lock

```ts
import { App } from 'morph'

App.requestSingleInstance((argv) => {
  mainWin.show()          // user double-clicked the icon: reveal, don't duplicate
  mainWin.navigate(routeFromArgs(argv))
})
```

Opt-in per app (a music player wants one instance; a text editor may genuinely want six windows). Without the lock, two processes share nothing — which is precisely the bug this prevents.

### Auto-start at login

```ts
App.setLoginItem({ openAtLogin: true, args: ["--minimized"] })
```

With `--minimized` support so tray-first apps don't flash a window on every reboot. Your users' autostart folders will thank you. (Their IT departments may not.)

### File drag-drop

```tsx
<div onDropFiles={(files) => importFiles(files)}>
  Drop invoices here
</div>
```

`files` arrives as real paths (sandbox rules apply per platform), with hover-highlight state for free. The current alternative — an `<input type="file">` — requires the user to *find* the file dialog, which is where imports go to die.

### OS theme listener

```tsx
import { theme, setTheme } from './themeStore'

SystemTheme.onChange((mode) => setTheme(mode))  // 'light' | 'dark', follows the OS
```

Ships with a `SystemTheme.current()` for first paint, so the app never flashes the wrong theme on launch. (The flash of blinding white at midnight has ended more app trials than any bug.)

### Badge & progress

```ts
Taskbar.setBadge(3)            // unread count on the dock/taskbar icon
Taskbar.setProgress(0.5)       // download/install progress under the icon
Taskbar.clearBadge()
```

Tiny API, huge "this app is alive" signal — the kind of thing users notice only when it's missing.

## Current state

| Piece | State |
|---|---|
| Window-scoped key events (focused node) | ✅ Shipped |
| Tray/hotkey/C++-driven flows via `native.cpp` | ✅ Shipped (untyped, manual) |
| Global hotkeys JS API | ❌ Not built |
| Protocol registration + URL forwarding | ❌ Not built |
| Single-instance lock | ❌ Not built |
| Login-item API | ❌ Not built |
| `onDropFiles` + drop highlight | ❌ Not built |
| `SystemTheme` listener | ❌ Not built |
| Badge / progress | ❌ Not built |

## Open questions

- **Hotkey conflicts** — return a result object, throw, or queue for the user's settings screen? (Result object: conflicts are data, not exceptions.)
- **Second-launch routing** — arbitrary argv → route mapping, or a fixed `onSecondLaunch(url)` handler per app?
- **Sandbox variance** — Flatpak portals, macOS entitlements, and Windows MSIX all gate different subsets; how much does the API normalize vs expose per-platform?
- **Ordering** — hotkeys + single-instance first (pure logic, no OS UI), drag-drop + theme next, badge last?

## Build steps (when picked up)

1. `Hotkey` register/unregister with conflict results (X11 → Win32 → Cocoa order, following Platforms)
2. Single-instance lock + second-launch argv forwarding as route props
3. Protocol registration in `morph package` + runtime URL handler
4. `onDropFiles` event + drop-target highlight state
5. `SystemTheme.current()` / `onChange`
6. Login-item API + badge/progress
7. Validation app: tray-first notes app summoned by hotkey, deep-linked, single-instanced
