# Magical CSS — Trick-Free Effects & Morph-Only Ideas

**Status:** future · **Priority:** low (possibility showcase, not roadmap order) · **Depends on:** [More CSS](more-properties.md), [Animations](animations.md), [Forge Renderer](../rendering/forge-tile-pool.md)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

## Read this first — basics come first

Morph supports almost none of the basics yet. Today only `border: solid` renders (`CSS::BorderStyle::{None, Solid}` in `runtime/cpp/style/css_enums.h`), there are no gradients, no `background-image` on boxes, no per-side borders, no `border-image`, single-value `border-radius` only.

An honest question this page must answer first: *why explore magical CSS when Morph doesn't yet support browser basics?*

**The team is building the basics first. This page is not a priority list — it is a possibility showcase. It records what becomes possible once Morph owns its own GPU shader and layout engine, free of decades of web compatibility baggage.**

Build order stays: solid basics (gradients → `background-image` → dotted/dashed → `border-image` → per-side borders → per-corner radius) *before* any magic below. Magic items reuse the same gradient/SDF plumbing, so designing them now keeps the basics from painting us into a corner.

## How to read each entry

Each effect shows:

- **CSS** — how it would look in Morph.
- **Browser today** — the `::before` / `radial-gradient` / `mask` / JS hack it needs.
- **Why browsers can't (or never will)** — spec, CPU fallback, compat, or privacy reason.
- **Cost** — `cheap` (one SDF math op), `medium` (extra uniforms / per-frame lerp), `hungry` (full-screen per-pixel loop — battery/GPU warning).

## A. Catch-up magic — browsers need tricks, Morph should do it natively (standard syntax)

### A1. All gradients + animation

```css
.linear { background: linear-gradient(45deg, red, blue); }
.radial { background: radial-gradient(circle at center, red, blue); }
.conic  { background: conic-gradient(from 45deg, red, blue); }
.repeat { background: repeating-linear-gradient(45deg, black 0 10px, white 10px 20px); }
.animated {
  background: linear-gradient(45deg, red, blue);
  animation: gradShift 3s infinite;
}
@keyframes gradShift {
  0% { background: linear-gradient(0deg, red, blue); }
  100% { background: linear-gradient(360deg, red, blue); }
}
```

Browser today: works, but animating gradients needs `@property --angle` registration hacks. Why limited: gradients are `<image>` paint, interpolation across different stop counts is undefined. Cost: `medium` — stop lerp per frame is fine; full-screen animated gradients are `hungry` (see battery table).

### A2. `background-image` on boxes

```css
.cover { background-image: url("hero.png"); background-size: cover; }
```

Browser today: trivial. Morph gap: `MORPH_FEATURE_IMAGE` only serves `<img>`, never CSS backgrounds. Cost: `medium` (texture memory + upload, no per-frame cost once cached).

### A3. Dotted / dashed / per-side borders

```css
.dotted { border: 2px dotted red; }
.perside {
  border-top: 4px solid red;
  border-right: 2px dashed blue;
  border-bottom: 1px dotted green;
  border-left: 3px solid black;
}
```

Browser today: trivial. Morph gap: single `float borderWidth` in `runtime/cpp/style/features/border.h`, single `BorderStyle`, `layout.cpp` does `borderWidth * 2`. Why it matters here: every magic border below builds on this. Cost: `cheap`.

### A4. Gradient border (standard `border-image`, no custom property)

```css
.grad-border { border: 4px solid; border-image: linear-gradient(red, blue) 1; }
```

Browser today: works via `border-image`, but breaks with `border-radius` and can't animate smoothly. Why browsers struggle: ring has no single direction, slice/repeat model predates gradients. Morph native: sample the same gradient evaluator on the ring band instead of the interior. Cost: `cheap` static, `medium` when animated.

### A5. Inverted / scoop radius

```css
.card { border-radius: 20px; }
.inner {
  border-top-right-radius: 30px;
  corner-shape: scoop;
}
```

Browser today: `::before { background: radial-gradient(circle at 100% 0%, transparent 100px, #dfdfdf 100px); }` + `::after { border-top-right-radius: 30px; }` with magic `100px` numbers that break on resize. Why browsers can't: `border-radius` is convex-only; concave needs boolean subtraction, only proposed as `corner-shape: scoop` draft. Morph native: `max(boxSDF, -circleSDF)` in `kQuadFragSrc` — one box, resizes correctly. Cost: `cheap`.

### A6. Gradient text

```css
.hero { background: linear-gradient(red, blue); background-clip: text; color: transparent; }
```

Browser today: needs `-webkit-background-clip: text` prefix dance. Cost: `cheap`.

## B. Morph-only magic — browsers don't have it, likely never will

### B1. Living materials — `aurora / plasma / noise`

```css
.card { background: aurora(#6d7cff, #a855f7, #22d3ee, speed: 6s); }
.grain { background: noise(opacity: .08) + linear-gradient(#111, #222); }
```

**What it does:** the box background is alive — slowly swirling aurora bands, plasma blobs, or film grain — computed per-pixel in the shader, no image file, no `@keyframes`, no JS loop. `speed: 0s` freezes one frame as a static poster.

**Browser today:** a `<canvas>` + WebGL/2D JS loop behind the content, or 3 stacked animated gradients with magic background-positions. Both need JS and break the moment you resize.

**Why browsers never will:** background paint must also run on CPU fallback with bounded battery cost. A per-pixel noise/octave loop has no cheap CPU path, so the spec will never bless it as a first-class `background` value.

**Cost:** `hungry` — full-box procedural loop every frame. Battery warning: hero cards only, auto-pause off-screen / window-blurred, honor `prefers-reduced-motion` with the frozen poster.

### B2. Cursor glow — no JS

```css
.glow { background: cursor-glow(200px, #6d7cff); }
```

**What it does:** the box lights up where the pointer is *inside it* — a soft radial glow that follows the cursor across the surface. Sweep a grid and every card ignites under your mouse. Zero event handlers, zero state.

**Browser today:** a `mousemove` listener writing `--mx/--my` CSS vars per card, one style recalc per mousemove, janky on grids.

**Why browsers never will:** paint is not allowed to know the cursor position without going through events (hit-test + fingerprinting cost). Morph's renderer already tracks the cursor for hover, so it just passes a `uMouse` uniform — radial falloff in the same SDF pass.

**Cost:** `medium` — one uniform + distance op per visible card; `hungry` on 100-card grids at 60fps (cap radius, repaint hovered card + neighbors only).

### B3. Rotating beam border — one line

```css
.beam { border: 2px beam(red, blue, 3s); }
```

**What it does:** an energy beam orbits the border forever — the glowing gradient outline seen on AI landing pages. Interior content is untouched; only the ring animates.

**Browser today:** `@property --a` registration + `conic-gradient(from var(--a), …)` + `@keyframes` + a wrapper div (because `border-image` can't animate). Four concepts for one effect.

**Why browsers are stuck:** gradient angles were never animatable in classic CSS, and the `border-image` slice model predates gradients entirely.

**Cost:** `medium` — one angle uniform lerped per frame on the ring band only.

### B4. Auto-meld tabs — scoops without manual masks

```css
.tabs { corner-meld: siblings 12px; }
```

**What it does:** two touching divs fuse into one shape — the concave joint between them is carved automatically, with zero `::before` masks and zero magic pixel numbers; joints survive resize because they are computed from real layout geometry.

**Browser today:** impossible in general — one hand-placed radial mask per joint, recomputed by hand on every redesign.

**Why browsers never will:** the box model forbids sibling-aware paint; a box may not know its neighbor's geometry. Morph's layout pass (`flatten.cpp`) already knows adjacency, so it hands the joint radius to the shader, which subtracts the joint circles like a scoop corner.

**Cost:** `cheap` — same circle-subtraction as A5.

### B5. Global light + materials — one scene, one light

```css
:root { light: top-left 120px; }
.clay  { material: clay; }
.glass { material: glass; }
.metal { material: metal; }
```

**What it does:** a single light source for the whole window. Every box shades itself consistently — soft top-left highlight, bottom-right shadow falloff — from its SDF normal. `material:` picks the response curve: soft clay, refractive-ish glass, sharp metal. Move the light, the whole UI relights.

**Browser today:** each `box-shadow` is hand-authored per element (`0 2px 4px …, 0 8px 24px …`); keeping 30 cards consistent is manual labor, and `neumorphism` recipes break the moment the background changes.

**Why browsers never will:** CSS has no scene concept — each box paints in isolation, and shadows must also render on CPU. A global light breaks that isolation contract.

**Cost:** `cheap` static (one diffuse evaluation from the existing SDF normal); glass refraction sampling would be `medium`.

### B6. Squish press — jelly buttons

```css
.btn:active { press: squish(0.12); }
```

**What it does:** on press, the button squashes with volume preservation — shorter *and* wider, like jelly — then springs back on release. `0.12` is how much height compresses.

**Browser today:** `transform: scale(.96)` shrinks content uniformly (text looks squeezed, not squashed) and releases linearly — no spring, no volume preservation.

**Why browsers never will:** transitions interpolate matrices linearly with fixed easings; spring physics with content-aware reflow was never part of the model.

**Cost:** `cheap` — non-uniform scale around `transform-origin` plus the existing spring driver in `style.cpp`.

### B7. Tilt-glass — 3D cards without JS

```css
.tilt { tilt: mouse 12deg; glare: on; }
```

**What it does:** the card tilts toward the cursor up to `12deg` in real 3D and a specular streak sweeps across it — the premium pricing-card effect, with no JS tilt library and no `perspective` wrapper divs.

**Browser today:** a JS tilt library writing `rotateX/rotateY` per mousemove plus a separate glare div.

**Why browsers won't:** it needs per-frame mouse → matrix plumbing that CSS alone cannot express; the JS bridge is the sanctioned path.

**Cost:** `medium` — matrix recompute + specular pass per frame while hovered; auto-disabled on low-power / reduced-motion.

### B8. Tail bubbles — borders that follow the tail

```css
.tip { tail: bottom 12px; }
```

**What it does:** a tooltip/speech bubble with a triangular tail merged into the *same* box — border, radius, gradient, and shadow flow around the tail continuously.

**Browser today:** `::after` rotated square overlapped on the edge — the border never actually follows the tail, so gradient/shadow tails are visibly faked.

**Why browsers never will:** pseudo-elements are separate boxes; a single continuous outline around box + tail needs boolean union of shapes, outside the box model.

**Cost:** `cheap` — triangle union in the SDF.

### B9. Squircle + ticket cuts — shape vocabulary

```css
.ios { corner-shape: squircle; corner-smoothness: .7; }
.coupon { edge-cut: left 12px, right 12px; }
```

**What it does:** iOS-style continuous corners (`squircle`, no more "almost round" approximation) and coupon notches (semicircle bites cut from the left/right edges) as one-liners.

**Browser today:** SVG `clip-path: path(…)` hand-drawn per size for squircles; `mask: radial-gradient(…)` arithmetic for ticket cuts — both size-fragile.

**Why browsers are slow here:** `corner-shape` is still a draft; masks were the escape hatch instead of first-class shapes.

**Cost:** `cheap` — exponent-blended SDF for squircle, circle subtraction for cuts.

### B10. Double / triple / inset borders

```css
.frame { border: 2px solid white; border2: 4px solid black; }
.innie { border: 3px solid red; border-inset: true; }
```

**What it does:** concentric border rings in one box, and borders that paint *inside* the box edge (eating into padding, never changing outer size) instead of expanding it.

**Browser today:** stacked `box-shadow: 0 0 0 4px white, 0 0 0 8px black` for multi-rings (which aren't real borders — no dashes, no gradient), and `outline-offset: -4px` / `box-shadow: inset …` approximations for inner borders.

**Why browsers never will:** `border` is defined as outside the padding box, full stop; a second border would need a second box model.

**Cost:** `cheap` — second ring in the same draw call; inset just draws the ring inside `sx,sy,sw,sh` (`rect.h`).

## Battery / resource warnings

| Effect | Cost | Why hungry | Mitigation |
|---|---|---|---|
| `aurora / plasma / noise` animated | `hungry` | per-pixel noise/octaves every frame | static poster when blurred, `speed: 0` / reduced-motion off, limit to 1–2 visible nodes |
| `backdrop-filter: blur()` | `hungry` | fullscreen framebuffer sample + blur taps | small radius, cache surface, Forge damage region only |
| `cursor-glow` on large grids | `medium` → `hungry` | per-card distance + repaints on every mouse move | radius cap, only hovered + neighbors repaint, throttle to frame clock |
| `beam` rotating border | `medium` | ring repaints every frame | pause off-screen, stop when tab hidden |
| `tilt + glare` | `medium` | transform + specular recompute | disable on low-power / reduced-motion |
| gradients static, scoop, per-side, dotted, inset, tail, squircle, cuts | `cheap` | one-time SDF math | none needed |

Rule: any `hungry` effect must ship with an automatic static fallback (window blur, off-screen, `prefers-reduced-motion`).

## Current state

| Feature | State |
|---|---|
| Gradients (all 6) + animation | ❌ Not started (`builder.rs` drops gradients via `parse_color`) |
| `background-image` on boxes | ❌ Not started (image pipeline is `<img>`-only) |
| Dotted / dashed / per-side | ❌ Not started (single `borderWidth`, `BorderStyle::{None, Solid}` only) |
| `border-image` with gradients | ❌ Not started (no custom property — standard syntax reserved) |
| Scoop / per-corner radius | ❌ Not started (single `border_radius: f32`, convex `sdRoundedBox` only) |
| Gradient text, patterns, shadows | ❌ Not started (`shadow.h` / `outline.h` dormant, see [More CSS](more-properties.md)) |
| B1–B10 Morph-only magic | ❌ Not started (needs A-group plumbing first) |

## Build order (basics first, magic later)

1. Gradients linear → radial → conic (+ repeating), static first then animation lerp.
2. `background-image` cover/contain on boxes.
3. Dotted/dashed, then per-side width/color, then `border-image`.
4. Per-corner radius + `corner-shape: scoop`.
5. Only then: B1–B10 behind `MORPH_FEATURE_*` flags with fixtures + screenshots.

## Decision log

- 2026-09-29: `border-gradient` custom property **rejected** — use standard `border-image: <gradient> 1` instead. No compat break.
- 2026-09-29: This page classified as possibility showcase, explicitly **not** priority over browser-parity basics. Basics (solid → dotted/dashed, gradients, images) ship first.
