# Mobile Support — Android & iOS

**Status:** future · **Priority:** medium · **Depends on:** [Platforms](../shipped/platform.md) (shipped), [Graphics APIs](../rendering/graphics-apis.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Morph on phones: the same `.mx` codebase compiling to a real Android APK and iOS IPA — no WebView wrapper, no JS engine shipped in the bundle, no second codebase in Kotlin or Swift. Desktop today, phones later, one language throughout.

## Why mobile? Our story.

Morph was never made for a platform first. Not Linux, not Windows, not macOS — and not Android or iOS either. Morph was made for **developers**. For the person who wants to write the syntax they love, ship something blisteringly fast, and never think about the runtime in between. Developer experience and peak performance — that order, both non-negotiable. Platforms are just where the apps land.

And look at what that philosophy already produced on desktop: fluid 60 fps apps compiled to **self-contained binaries as small as ~162 KB** — measured on the hello-size fixture, not a slide deck (see [Lean Binaries](../tooling/lean-binaries.md)). You write your favorite syntax — JSX, TypeScript, the event model and CSS you already know from the web — and out comes native code with no interpreter, no virtual machine, no 100-megabyte sidecar. Things other frameworks either can't give you at all, or can only give you with strings attached.

Because look at what the alternatives actually ship. React Native ships a full JavaScript engine *inside every app* — Hermes or JSC booting up, megabytes of runtime warming up, just so your buttons can render. Tauri and Capacitor hand rendering to the OS WebView — which can easily consume 100 MB+ of RAM just to show a hello world, because you're not shipping an app anymore, you're shipping a browser tab with commitment issues. A whole engine, or a whole browser, per app, on every user's device — all of it overhead nobody asked for, all of it justified in the name of developer experience. But here's the thing: if the DX costs every user 100 MB of RAM and every developer a second runtime to debug, *it isn't good DX*. It's a loan with the interest paid by someone else.

Morph refused that loan on desktop. Same code you love writing, compiled straight to native — no engine, no WebView, no middleman taking a cut of your RAM and your startup time. And it worked. So now the honest question: **if the formula works on desktop, why not phones?** A phone is the same problem in a smaller box with a stricter landlord (the OS). The architecture doesn't change — signals are still signals, the layout engine still lays out, AOT still deletes the runtime. What's hard is the last mile: touch input, lifecycles the OS owns, store review gates. Hard, not impossible. Doable — especially with real apps pulling it forward.

That's where you come in. If you're building something that needs phones — if the desktop app is done and the emails asking "Android when?" are piling up — tell us. Open an issue, send a suggestion, show us the app. Roadmap pages describe what *we* think; issues describe what *you* need, and need wins. See [Suggestions](../tooling/suggestions.md) — one paragraph (what you're building, what's missing, what you tried) is enough to move this page from "planned" to "in progress."

## Why phones at all? Isn't Morph a desktop framework?

It is today. But there are three billion smartphones in active use and that number is still growing, while the desktop count sits flat. For entire categories — notes, chat, fitness, field tools, payments — the phone isn't a port of the real app. It *is* the real app, and the desktop version is the companion. A framework that can't reach phones caps every app built with it at the desk.

Where mobile is the product, not the port:

| Category | Phone role | Desktop role |
|---|---|---|
| Notes / todos | Capture on the go — the app | Review station |
| Chat / messaging | The app, full stop | Companion |
| Field data collection | The *only* computer on site | Dashboard |
| Fitness / health | Sensor lives on the body | History charts |
| Payments / POS | Counter terminal | Back office |

The binary-size story gets even better on mobile. On desktop, users download your 150 MB app once on Wi-Fi and forget it. On phones, users stare at a storage-full warning deciding which app to delete — and they delete the big ones without reading the description. A 5–10 MB Morph APK competates against products ten times its size on the one metric checked before installing.

And the cheapest phones on earth are Morph's natural habitat: 2 GB RAM Androids punish JIT warm-up and GC pauses with exactly the jank users describe as "this app feels cheap." AOT-compiled signals, no engine, no collector. The architecture those phones are begging for — the framework just doesn't ship it yet.

## Why not yet? What's actually blocking?

Mobile is not "desktop with a smaller window." Seven blockers, each with a known solution — listed so "why not yet" has line items instead of vibes:

| # | Blocker | Why it's real | Unlocks |
|---|---|---|---|
| 1 | No mobile windowing | GLFW is desktop-only. Android needs `NativeActivity` surface lifecycle; iOS needs `UIViewController` + layer host. `core/window.*` gets a second backend. | Everything |
| 2 | GLES / Metal, not GL | Mobile GPUs speak GLES 3.x; Apple deprecated GL entirely. | Rendering at all |
| 3 | Desktop queue first | Win/mac share ~90% of the desktop stack; mobile shares ~40%. Sequencing, not stalling. | Windows, macOS shipped |
| 4 | Touch input universe | Tap, long-press, swipe, pinch, kinetic scroll, keyboard-avoidance. Mouse machinery transfers ~nothing. | Usable apps |
| 5 | OS-owned lifecycle | The OS kills your process and expects full restoration. `morphShared` dies with the process today. | Tombstoning story |
| 6 | Store gatekeepers | Play review, App Store review, entitlements, privacy manifests. | Distribution |
| 7 | Mobile CI | Emulators, device farms, per-ABI builds (arm64-v8a, x86_64 emulator). Unglamorous, non-optional. | Confidence |

**Decision rule:** desktop platforms → graphics backends → Android shell → touch → iOS shell → sensors → stores. Each step independently shippable, in that order, no skipping — doing iOS first secretly funds macOS Metal, which is either clever sequencing or scope creep wearing a trench coat.

## What changes for me? Show me the before/after.

**Before** — the email every desktop-only dev knows:

```tsx
// ❌ Today: one codebase, one form factor
morph build                    // Linux binary. Phones don't exist.
```

**After** — same files, two new flags:

```bash
# ✓ Phones: same .mx files, new targets
morph build --target android-arm64    # APK (AAB for Play submission)
morph build --target ios-arm64        # IPA via Xcode project export
```

Three developers, three release days:

**Meera's notes app.** Fifty thousand Linux/Windows users, weekly "Android when?" emails. That email used to mean *learn Kotlin, hire someone, or rewrite in Flutter and maintain two codebases forever*. Now: one afternoon adding responsive rules, a `route.mobile.mx` for the two screens that genuinely differ (desktop data-table → mobile card list), an icon manifest entry, a Play Console upload. The thread that said "sorry, desktop only" now says "beta link inside." Porting tax drops from *a second team* to *an afternoon plus paperwork*.

```tsx
// ✓ Divergent screens opt out per route — everything else adapts
// settings/route.mx          → desktop card + sidebar
// settings/route.mobile.mx   → full-screen scrolling list (mobile only)
```

**Devon's React Native migration.** Screen by screen: each `.mx` route replaces one RN screen, shared TypeScript moves over nearly verbatim. The release that converts him: APK 34 MB → 9 MB, cold start 2 s → under 0.5 s, and upgrade season *stops existing* — no native-module matrix to break, because there are no native modules in his app. Frameworks are adopted on promises and kept on release days.

**Priya's field fleet.** Ten thousand 2 GB-RAM Androids. Her crash reports go *down* after launch — bridge-timing races and OOM kills are two bug categories that don't exist in an AOT binary without a bridge. She doesn't care about the philosophy. She cares that ten thousand underpowered phones run all day without dying.

## How do windows become screens?

The elegant part: mobile apps are single-"window" by nature, and Morph already has every concept the screen stack needs — just wearing desktop clothes.

| Desktop concept | Phone meaning |
|---|---|
| `new Window("/settings")` | Push a screen |
| `win.navigate("/x")` | Push / replace |
| System back button | `navigate(-1)` with OS chrome |
| `navigation.cache` | Scroll-preserving back stack (return *where you were*, not the top) |
| Per-window `morphShared` | Per-screen-instance isolation, free |

```tsx
// ✓ Two drafts of the same form on different screens never leak —
// same reason two desktop windows don't. No new API to learn.
const a = new Window("/compose", { data: { draftId: 1 } })
const b = new Window("/compose", { data: { draftId: 2 } })
```

**Decision rule:** every `new Window()` is a pushed screen; dialogs/sheets stay overlays. Call site decides, route config refines — same split as desktop ownership.

## What does touch look like?

The runtime owns gesture recognition (not the OS), so a swipe means the same thing on Pixel and Galaxy — ending the "works on Pixel, weird on Galaxy" genre at the source:

```tsx
<div
  onTap={() => open(item)}
  onLongPress={() => select(item)}
  onSwipeLeft={() => dismiss(item)}
  onPinch={(scale) => zoom(scale)}
>
```

| Gesture | Fires when | Notes |
|---|---|---|
| `onTap` | Quick down-up, no movement | Tap-vs-scroll decided in ~10 ms or the UI feels laggy |
| `onLongPress` | Held past threshold | Selection, context actions |
| `onSwipeLeft/Right/Up/Down` | Fling with velocity | Dismiss, archive, navigate |
| `onPinch` | Two-finger scale | Zoom, with scale factor |

Kinetic scrolling ships with platform-tuned friction (iOS glides longer, Android stops sooner — users feel it even when they can't name it). Pull-to-refresh is a declaration on a scroll container, not a library you evaluate, integrate, and maintain through three majors.

## What about notches, home bars, keyboards?

Layout inputs, not props you thread. The engine knows the intrusions; styles reference them:

```tsx
// ✓ Punch-hole cameras and Dynamic Islands are the engine's problem
<div style={{ paddingTop: 'env(safe-area-inset-top)' }}>
  <text>Never under a camera. On any Samsung. Ever.</text>
</div>
```

| Input | Source | Covers |
|---|---|---|
| `env(safe-area-inset-*)` | OS | Notch, home indicator, status bar |
| `env(keyboard-inset-height)` | IME | Bottom sheets above the keyboard |
| Auto scroll-into-view | Engine | Focused input never hides under the keyboard |

The keyboard rule every chat app hand-rolls — listen, measure, shift, scroll, get it slightly wrong on one platform — becomes engine behavior. Anything every app hand-rolls badly becomes something the engine does once, correctly.

## What about camera, GPS, push? Do I write native code?

No. Same convention as [Native Modules](native-modules.md) — config-object constructors, only imported modules ship:

```ts
import { Haptics, Geolocation, PushNotifications, ShareSheet } from 'morph/mobile'

Haptics.tap('light')                    // one line, both platforms
const pos = await Geolocation.current()
await PushNotifications.register()      // FCM/APNs token, zero native code
ShareSheet.share({ title, url })
```

Permissions are declared in `morph.config.json` — a missing declaration fails the **build**, not review. App Store rejections arrive at 2 AM with a screenshot; build errors arrive now, with coffee:

```tsx
// ✓ "Denied" is a value, never a crash or a spinner of doom
const pos = await Geolocation.current()
if (pos.denied) return <text>Enable location in Settings to see nearby stops.</text>
```

| Module | Ship order | Why there |
|---|---|---|
| Haptics, ShareSheet | First | Pure logic, no permissions drama |
| Geolocation, Camera | Next | Permission-gated, typed deny results |
| PushNotifications | Last | Server side (FCM/APNs) + tokens + review scrutiny |

## What are the others doing right now — and where does it hurt?

Credit where due: developers choose incumbents for reasons. Each entry: what users genuinely love, the specific structural pain, and Morph's bet.

**React Native — users love:** huge ecosystem, hireable devs, CodePush OTA updates that skip review, and the new architecture (Fabric + TurboModules + Hermes) fixing the worst bridge jank.

```tsx
// The shape that never went away: JS on one side, native on the other,
// still coordinating across a boundary on every interaction
```

**Where it hurts:** upgrade season. Every minor version moves something under community native modules; "works on 0.72, afraid to touch it" is a production-team proverb. 25–35 MB hello-worlds are normal. Morph's bet: AOT removes the *categories* — no bridge, no bridge-timing bugs, no engine upgrade seasons. RN's costs are structural; structural costs don't get fixed, they get managed, forever.

**Flutter — users love:** custom renderer proving one-codebase works, best-in-class hot reload, Impeller fixing shader jank. The closest cousin, and the proof the thesis sells.

**Where it hurts:** the floor. Dart VM + engine + framework ≈ 20–30 MB before your code; the GC has frame-time opinions surfacing exactly on cheap Androids — most Androids. Morph's bet is narrower: same philosophy, signals instead of rebuilds, no GC, a third of the binary. On a flagship Pixel nobody can tell. On a $90 phone everybody can.

**Tauri Mobile / Capacitor — users love:** your existing web app, on both stores, this afternoon. Unbeatable for porting.

**Where it hurts:** the ceiling is web rendering — scroll physics never quite native, 60 fps achievable but never free, WebView version skew reintroducing per-device folklore through a new door. Morph isn't competing for that job; it's for apps that want to *not be web pages*.

**Kotlin Multiplatform — users love:** shared logic in Kotlin, sane default for Kotlin shops, JetBrains executing well.

**Where it hurts:** iOS is young, interop seams show, and UI boundaries still need per-platform code plus a Gradle build that has ended friendships. Morph shares the UI layer itself, not just the logic under it.

**Native (SwiftUI / Jetpack) — users love:** day-one OS features, every WWDC afternoon.

**Where it hurts:** headcount and drift — two codebases becoming *different apps* sharing a name. Morph's pitch was never "better at native things." It's "one team ships everywhere."

| | React Native | Flutter | Tauri Mobile | Capacitor | Morph (planned) |
|---|---|---|---|---|---|
| UI language | JS + native | Dart + renderer | HTML in WebView | HTML in WebView | TSX → native |
| Runtime shipped | Hermes/JSC | Dart VM (~20 MB floor) | OS WebView | OS WebView | **None — AOT** |
| Hello-world | ~25 MB | ~20–30 MB | ~3–5 MB | ~5 MB + WebView | **~5–10 MB** |
| Updates | OTA JS (CodePush) | Store | Store | OTA web assets | Store (signed) |
| Desktop story | Separate (Electron) | Separate | ✅ Same project | None | **Same `.mx` files** |

**Decision rule:** iOS-only → SwiftUI, sincerely. Web app to port → Capacitor/Tauri. One team, everywhere → that's the gap Morph aims at.

## What does the framework ask of me?

Same files, a short contract:

| Rule | Enforcement |
|---|---|
| Touch targets ≥ 44pt | `morph check` warns in mobile builds (warns, not rejects — your UI, your funeral, announced in advance) |
| Divergent screens → `route.mobile.mx` | Convention; adaptive middle uses breakpoints |
| Offline is normal control flow | `fetch()` failures render states, not crashes |
| Denied permissions render fallbacks | Typed results; denial is a value |
| Background work declares itself | Config-declared timers/location; OS throttles the rest |

## Current state

| Piece | State |
|---|---|
| Desktop Linux / Windows / macOS | ✅ Shipped |
| GLES / Metal rendering backends | ❌ Planned ([Graphics APIs](../rendering/graphics-apis.md)) |
| Android `NativeActivity` shell | ❌ Not built |
| iOS `UIViewController` shell | ❌ Not built |
| Touch/gesture recognizer | ❌ Not built |
| Safe-area / keyboard-inset layout inputs | ❌ Not built |
| Mobile sensor modules | ❌ Not built (convention exists: [Native Modules](native-modules.md)) |
| NDK/Xcode in `morph doctor` + mobile CI | ❌ Not built |
| Play (AAB) + App Store (IPA) packaging | ❌ Not built |

## Open questions

- **Android or iOS first?** Android: market share, NDK + GLES, sideloading with no gate. iOS: paying users, forces the Metal backend macOS also needs.
- **Back-stack semantics** — every `new Window()` a pushed screen, or dialogs stay overlays? Call site or route config owns it?
- **Tablets & foldables** — breakpoints cover them, or a posture API (fold state, span)?
- **OTA updates** — asset-only OTA on Android, or full-binary everywhere for symmetry? (iOS forbids silent code updates.)
- **Minimum hardware** — GLES 3.0 baseline (~99% since 2014)? Does `morph check` flag shaders above it?
- **Watch / TV?** — No. Phones first, tablets free with responsive layout.

## Build steps (when picked up)

1. GLES backend via [Graphics APIs](../rendering/graphics-apis.md) (Android unblocks here)
2. Android shell: `NativeActivity` + surface lifecycle + NDK toolchain + arm64 CI job
3. Touch/gesture recognizer + kinetic scroll + safe-area/keyboard layout inputs
4. Back-stack navigation mapping (`Window` → screen stack, OS back button)
5. Metal backend → iOS shell (`UIViewController`, Xcode export, device CI)
6. Sensor modules (Haptics → ShareSheet → Geolocation → Camera → Push)
7. Play (AAB) + App Store (IPA) packaging, signing, submission docs
8. Validation: one responsive notes app on all five targets from a single codebase
