# WebView — Embed HTML via the OS WebView

**Status:** future · **Priority:** medium

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

A `<morph-webview>` element: render real HTML/CSS/JS content inside a Morph window — powered by the **OS's default webview engine**, exactly like Tauri. No Chromium bundled, no browser shipped with your app.

| Platform | Engine |
|---|---|
| Linux | WebKitGTK |
| Windows | WebView2 (Edge) |
| macOS | WKWebView |

## Why it matters

- **Render web content you didn't build in Morph** — dashboards, charts, documentation, rich text, third-party web pages
- **Tauri-style hybrid apps** — the native shell in Morph (windows, menus, tray, native rendering), the content as HTML. *With this, what Tauri does, Morph can do too* — while the rest of your app stays compiled native
- **The best of both** — your app is native; the *content that is genuinely web* is web
- No extra runtime weight — the OS already ships the webview; your binary stays small

## How it will work

```tsx
{/* a full web page inside a native window */}
<morph-webview src="https://example.com" width="100%" height="100%" />

{/* or local HTML shipped with your app */}
<morph-webview src="./docs/index.html" width="640" height="480" />
```

The exact tag may differ (`<webview>`, `<morph-webview>`, …) — the design is what matters: a native element that hosts the OS webview and treats it like any other node in the layout tree.

### Attributes

- `src` — URL or local file to load
- `width` / `height` — layout, like any element
- `sandbox` — restrict scripts, forms, popups, same-origin rules
- `userAgent` — override the user agent
- `transparent` — let the webview background be transparent over the app

### JS APIs

A handle to control the webview from component logic, two-way:

```ts
const web = useRef<morph-webview>()   // or however handles resolve

web.loadURL("https://example.com")
web.loadHTML("<h1>Hello from Morph</h1>")

const title = await web.evaluate("document.title")   // run JS inside the webview

web.on('navigate', (url) => { ... })   // webview → app events
web.on('message', (msg) => { ... })    // content posts a message to the app

web.goBack() / web.goForward() / web.reload()
```

**App → webview:** `loadURL`, `loadHTML`, `evaluate` (run arbitrary JS inside the page).
**Webview → app:** events — navigation, page load, and a message bridge so the HTML page can talk to your Morph logic.

## What this enables

- **Hybrid apps** — native shell + web content, Tauri-style
- **Charts & dashboards** — the entire web chart ecosystem (ECharts, Chart.js, D3) where Morph's own renderer would be overkill
- **Docs & rich content** — render markdown/HTML documentation, emails, reports
- **Embedded web** — maps, payments, login flows that are already web services
- **Gradual migration** — move an existing web app to the desktop by wrapping it, then slowly replace screens with native Morph

## When NOT to use it

The webview is a complement, not the product. Building your *whole app* in a webview means you're back to shipping a browser experience — the exact thing Morph exists to avoid. Morph's selling point is that your UI is compiled native; the webview is for the parts that are *genuinely web*.

## Security notes

- **No bundled browser** — the OS webview updates with the OS (like Tauri), a smaller and faster-patched attack surface than shipping Chromium
- **Sandboxing by default** — no Node/bridge access from inside the page unless explicitly enabled
- **Isolation** — the message bridge is the *only* channel between the page and your app; nothing else crosses

## Current state

| Piece | State |
|---|---|
| OS webview integration (WebKitGTK / WebView2 / WKWebView) | ❌ Not built |
| `<morph-webview>` element + layout integration | ❌ Not built |
| `loadURL` / `loadHTML` / `evaluate` / events | ❌ Not built |
| Sandbox + message bridge | ❌ Not built |

## Open questions

- **Engine choice per platform** — WebKitGTK vs WebView2 vs WKWebView behave differently; how much behavior do we normalize in the element API?
- **Bridge design** — JSON messages? Promises? How do `evaluate` results come back (async)?
- **Performance** — a webview inside a GPU-rendered layout tree needs compositing; where does the webview surface sit in the render pipeline?

## Build steps (when picked up)

1. `morph-webview` element: native node hosting the OS webview
2. `src` loading (URL + local file) with layout integration
3. `loadURL` / `loadHTML` / `reload` + navigation events
4. `evaluate` + message bridge (two-way)
5. Sandbox attributes + security review
6. Validation app: native Morph shell + embedded dashboard (charts) + a docs page