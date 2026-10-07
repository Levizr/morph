# Animation Engine: Chrome vs Morph — Why Morph Is Shaped This Way and What We Fix Next

**Status:** future · **Priority:** medium · **Depends on:** [Animations](animations.md) (choreography layer), [Performance](../rendering/performance.md) (compositor interpolation), [Forge Tile Pool](../rendering/forge-tile-pool.md) (damage-aware animation)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

We write this down as a team (Oct 2026): how Chrome animates vs how we animate in Morph, why our engine looks the way it does (static AOT vs dynamic browser), why keyframes and transitions are two separate implementations, what it actually costs us to make one more property animatable, and what we plan to change — and deliberately not change.

Companion user docs: `docs/css/animations.md`, `docs/css/transitions.md`.

## 1. Summary

- Chrome and Morph share the same **shape**: declarative CSS → timeline → per-frame interpolation → vsync present, with a main/compositor thread split and property promotion rules.
- Chrome's compositor is a **full animation engine** (own clock, keyframe data, property trees, layer cache). Morph's compositor is a **late-stage smoother**: the main thread still authors every frame (`updateCssAnimations` → `markDirty` → `commitFrame` → `renderFrame`).
- That gap is **not** an oversight to fix by copying Chrome. Chrome's complexity is forced by arbitrary runtime input. Morph is AOT: the full animation set is known at `morph build` time. The correct fix is to push decisions to **build time** (lint, classify, bake) — something Chrome cannot do.
- Keyframes and transitions stay **separate engines**. They are different algorithms, and Morph has no imperative API (`element.animate()`) forcing them into one object model the way Chrome's Web Animations API does.
- Adding one property today costs ~9 small additive edits (~1 hr). A full property-table refactor is **not** worth it yet (break-even ≈ 15–20 properties). What *is* worth it now: a consistency test, a `Count` enum sentinel, and a build warning for silently dropped properties (~half a day total).
- Full timing-function support ships with this change: spec-exact keyword easings plus `cubic-bezier()`, `steps()`, and per-keyframe `animation-timing-function` — the silent fallback to quadratic approximations ends here.
- We evaluated Qt's animation approach and decided against adopting it wholesale; we borrow one principle from it (interpolate by value type, resolved at compile time), recorded in §13.

## 2. How Chrome animates

### 2.1 One timing model for everything (Web Animations)

Blink's `core/animation/` implements the **Web Animations timing model**, which drives CSS Animations, CSS Transitions, *and* the JS Web Animations API (`element.animate()`). Per Blink's own README: the main-thread animation engine "drives CSS Animations, Transitions and exposes the Web Animations API."

Each running animation is an object (`Animation` + `KeyframeEffect`) holding:

- keyframe list with per-keyframe and per-iteration `animation-timing-function`
- full `cubic-bezier(x1,y1,x2,y2)` / `steps()` timing (spec-exact math)
- a **timeline** (document timeline = monotonic clock; or a `ScrollTimeline`)

Timing (delay, iterations, direction, fill) is therefore **property-blind** — one implementation serves all three animation sources.

### 2.2 Per-property support is a declarative flag, not per-feature code

No Chrome dev writes "hover animation for `foo`" or "keyframes for `foo`". Those engines are generic. Per-property work is tiered:

| Tier | Cost to add property `foo` | Mechanism |
|---|---|---|
| Timing / `:hover` / `:active` / `@keyframes` triggering | **0** | Generic engines; pseudo-class flip just re-runs style and diffs |
| Smooth blending | **flag + registration** | `interpolable: true` in `third_party/blink/renderer/core/css/css_properties.json5`, plus registration against a (usually generic) `InterpolationType` by value kind (number/length/color). Complex values (grid tracks, `box-shadow` lists, `clip-path`, transform function lists) need a dedicated `XxxInterpolationType` |
| Discrete fallback | automatic | `interpolable: false` still animates — it flips at 50% per spec |
| Compositor (off-main-thread) support | **explicit code** | Allowlist in `CompositorAnimations` + cc-side support + per-property fallback-reason table (there is open tech debt here upstream: "Define compositor fallback reasons per property"). Only `transform`, `opacity`, `filter`, `translate`/`rotate`/`scale` qualify |

Legacy WebKit had the list-form explicitly: `CSSPropertyAnimation.cpp` with a ~2000-line `gPropertyTable[]` mapping property → blend functions. Modern Blink refactored that into the reusable `InterpolationType` system — same idea, less duplication.

### 2.3 The compositor is a complete loop

For compositor-driven animations:

1. Blink **proposes** the animation to the compositor; the compositor **accepts or rejects** it (fallback = main thread).
2. Accepted animations live on cc's side (`AnimationHost` / animation nodes), sampled by cc each vsync.
3. Sampled values go into **property trees** (`TransformPropertyNode`, `OpacityPropertyNode`, clip/scroll nodes), not per-node offsets — a parent's animated transform automatically carries descendants via parent-chain recomputation.
4. Animated content gets **promoted layers** (cached rastered textures; `will-change` forces it). Raster runs once; only the draw phase re-runs per frame.

Consequence: a 100ms main-thread JS stall drops **zero** composited animation frames. Main-thread (layout) animations freeze and then jump to the correct position, because time is absolute.

### 2.4 Absolute clock, not accumulated deltas

Chrome samples by asking the timeline "what is `currentTime` now?" on every vsync `BeginFrame`. Missed frames don't drift. `requestAnimationFrame` callbacks likewise receive the vsync timestamp.

## 3. How Morph animates (C++ side)

### 3.1 Pipeline (declaration → pixels)

```
style.css (@keyframes + animation/transition + :hover) imported from App.mx
  → lightningcss parse (crates/morph-parser/src/css_parser.rs)
  → IR (crates/morph-ir/src/node.rs: IRAnimation/IRKeyframe; builder.rs: is_animatable, parse_animation_*, parse_transition)
  → C++ codegen (crates/morph-codegen/src/node_emitter.rs: emit_animations → CssAnimation{...}, morphAddKeyframe() registry)
  → main loop (app_main.cpp.tera: dt clamped 0.1s, glfwWaitEventsTimeout(0.016))
  → WindowManager::pump(dt) → MorphNode::update(dt)
  → commitFrame() (layout + paint + flatten to RenderFrame)
  → compositor thread interpolates AnimationState → renderFrame() draws → glfwSwapBuffers (vsync)
```

### 3.2 Keyframes driver (`runtime/cpp/core/node/animation.cpp`)

`MorphNode::updateCssAnimations(dt)` per animation config:

1. Clocks: `elapsed += dt` if `running` (`paused` freezes). `p = elapsed - delay`; `p < 0` samples only under `Backwards`/`Both` fill.
2. Position: `iterPos = p / duration`; finished (`iterPos >= iterations`) either holds `sample(iterations - 1e-5)` under `Forwards`/`Both` (fractional tails like `2.5` stop mid-iteration) or reverts via `restoreProps(base)` under `None`/`Backwards`.
3. Direction map (`Normal` / `Reverse` / `Alternate` / `AlternateReverse`), then `applyEasing(t)` — today **quadratic approximations** (`t*t`, `1-(1-t)^2`) of the five keyword easings; `cubic-bezier()`/`steps()` fall back at build time. Phase 2 (§7) replaces this with spec-exact easings plus full `cubic-bezier()`/`steps()` support, shared across all three call sites.
4. Per-property `sampleProperty()` implements CSS Animations L2: interpolate between two setters, hold the last, keep the underlying value before the first.
5. `resolveValue()`: `%` resolves against the parent box (`width`/`left`), own `min(w,h)` (`border-radius`); `px`/numbers are build-baked. `applyValue()` writes `MorphStyle` and marks `PaintDirty` (or `LayoutDirty` for `width`/`height`/`left`/`top`/`font-size`/`border-width`).
6. Special cases: `Transform` tries op-list interpolation (preserves `rotate(0deg)→rotate(360deg)`) then falls back to `mat4Interpolate()`; gradients `lerpGradient()` when compatible, else discrete flip at `f >= 0.5` (browser behavior).
7. Any active state sets `m_isTransitioning`/`m_hasLayoutTransition`, which forces `commitFrame()` that frame.

### 3.3 Transitions driver (`runtime/cpp/core/node/style.cpp`)

Separate implementation, whole-style snapshots:

1. `onHover/onActive` → `_retargetState()`: swaps the `animations` list instantly (hover animations run only while hovered), snapshots `pre/pressStyle`, builds `HoverTransition{start=current, target}` when `m_transitionDuration > 0`, else snaps.
2. Per frame `_updateStateTransition()`: `interpolateStyles(style, start, target, easing(t))` — lerps colors/opacity/padding/radius/font-size, lerps lengths only when units match (else snaps to target, like browsers), lerps matrices via `mat4Interpolate()`, snaps discrete fields (`display`, `flex-direction`, `font-weight`).
3. Release path `buildReleaseStyle()` unwinds OFF states in reverse delta-stack order; **in-flight transitions snap to target first** so the equality checks unwind cleanly under rapid clicks (`style.cpp:402`).
4. Ancestor-hover (`.parent:hover .child`) has its own parallel `AncestorHoverTransition` state machine.

### 3.4 Compositor half-loop (`runtime/cpp/core/compositor.cpp`, `render_frame.h`)

`commitFrame()` flattens to `RenderFrame{nodes: FlatRenderNode, animations: AnimationState{from,to,startTime,duration}}` and sets `framePending`. The compositor thread waits on `framePending`, computes `t = (now - start) / duration` with the same quadratic easing, writes `animOffsetX/Y`, `animOpacity`, color channels onto `FlatRenderNode`, pushes completion events to `g_feedbackQueue`, sets `frameInterpolated`. `renderFrame()` spins on `frameInterpolated`, draws at `x + animOffsetX`, then `glfwSwapBuffers()` under `glfwSwapInterval(1)`.

This is a **half-loop**: the compositor interpolates a handed `from→to` pair, but clock advance, style writes, flatten, and present all still ride the main thread every animated frame.

### 3.5 Code map

| Layer | File |
|---|---|
| Parse | `crates/morph-parser/src/css_parser.rs:38`, `ast_types.rs:361` |
| IR types + allowlist + lowering | `crates/morph-ir/src/node.rs:153,180`, `builder.rs:4046,2904,5445-5670` |
| Codegen (registry + configs) | `crates/morph-codegen/src/node_emitter.rs:2282,2824,2866,2895`, `feature_set.rs:156` |
| Runtime types + registry | `runtime/cpp/style/features/animation.h:19,66,94` |
| Keyframe driver + easing + sampling | `runtime/cpp/core/node/animation.cpp:16,121,326` |
| Transitions + `interpolateStyles` + `update()` | `runtime/cpp/core/node/style.cpp:6,101,266,376,575,609,824,854` |
| Node state | `runtime/cpp/core/node.h:43,64,174,202,457,481,530` |
| Frame channel + compositor anims | `runtime/cpp/core/render_frame.h:15,23,104,181,201` |
| Compositor loop | `runtime/cpp/core/compositor.cpp:28` |
| Commit / present / pump | `runtime/cpp/core/window.cpp:909,1414`, `window_manager.h:727` |
| Dev hot-reload path | `runtime/cpp/dev/ir_deserializer.h:120` |

## 4. Why Morph is shaped this way (static AOT vs dynamic browser)

Chrome's runtime complexity is **forced** by not knowing the program:

| Chrome must do at runtime | Why it can't be build-time |
|---|---|
| Delegation handshake (accept/reject + fallback) | Page + CSS + JS arrive at runtime; support must be discovered |
| Layer promotion heuristics + eviction | Which elements animate is unknown until execution |
| Full spec (`cubic-bezier`, `steps`, per-keyframe timing, WAAPI) | Any site may use any of it; can't drop features |
| Property-tree recompute | DOM mutates at runtime |
| Post-hoc audits ("avoid non-composited animations") | Can only warn after the page shipped |
| Ship the whole engine to every page | Page 1 might need it; no per-site dead-code elimination |

Morph knows the **complete, final animation set** at `morph build` time. We already exploit this in four places:

1. **Feature dead-code elimination** — `#ifdef MORPH_FEATURE_ANIMATION` (`animation.h:10`): apps without animations ship zero animation code. Chrome ships the machinery to everyone.
2. **Baked keyframes** — parsed in Rust, emitted as `morphAddKeyframe()` C++ instead of runtime-parsed objects.
3. **Compile-time accept/reject** — `is_animatable()` (`builder.rs:4046`) *is* Chrome's delegation handshake, run once at build. No negotiation needed at runtime.
4. **Baked numeric values** — colors/numbers become C++ floats at codegen; only `%`/transform/gradients stay raw CSS.

Not yet exploited (the gap we plan to close): build-time **classification** (compositor-routed vs layout, with warnings), build-time **diagnostics** for dropped/unsupported constructs, and deterministic test clocks. All are free *because* the animation set is closed — Chrome can never have them.

What AOT does **not** buy: immunity to main-thread jank (threading physics are unchanged), or freedom from bounded dynamism (reactive state, dev hot-reload via IR JSON).

## 5. Why transitions and keyframes are separate — and stay separate

They are **different algorithms**, not two spellings of one thing:

- **Keyframes**: per-property timeline sampling against a global registry (`sampleProperty` per prop, L2 hold/underlying semantics, per-animation clock, fill/revert bookkeeping).
- **Transitions**: whole-style snapshot lerp (`startStyle → targetStyle`) with retarget-on-event and release-unwind semantics (pre/press snapshots, reverse-stack revert, snap-to-target-then-retarget under rapid clicks, reactive-effect interplay via `interruptStateTransitions`).

We keep them separate because merging would mean reimplementing the transition unwind logic on top of per-property timelines — putting tested, subtle behavior (the rapid-click freeze fix, effect-vs-state precedence) at risk for zero user-visible gain.

**Why Chrome unified them anyway:** Chrome *had* to. The Web Animations API (`element.animate()`, pause/seek/reverse, `finished` promises) forces transitions and animations into one object model — a `CSSTransition` must be pausable and seekable like any `Animation`. The unification cost is justified by the imperative API.

**We have no imperative animation API** — no `animate(el)`, no timelines, no pause/seek handles. Nothing forces the merger. The two declarative paths (`animation` runs on a clock; `transition` runs on a state change) map cleanly onto two implementations.

**Our decision as a team: we do not merge.** Not now, not later, unless we add an imperative API — in which case we revisit with the API design, not before.

## 6. Adding animation support for one property: exact effort

Prerequisite (already paid when the property became styleable): a `MorphStyle` field plus `apply_css_prop`, serializer entries, `applyStyleDelta`, `interpolateStyles`, `buildReleaseStyle`, `hasLayoutDiff` — i.e. anything styleable already transitions on `:hover` for free.

Animation-specific work for e.g. `letter-spacing` — ~9 small additive edits, ~1 hr including the fixture run (`animation-test`, `animation-test-2.0`, `border-state`):

| # | File | Change |
|---|---|---|
| 1 | `crates/morph-ir/src/builder.rs:4046` | add to `is_animatable()` allowlist — **miss this and the property is silently dropped** (`builder.rs:2916`) |
| 2 | `runtime/cpp/style/features/animation.h:19` | add `KeyframeProperty` enum member |
| 3 | `runtime/cpp/core/node/animation.cpp:51` | `resolveValue()` case: value → float, `%` base selection |
| 4 | `runtime/cpp/core/node/animation.cpp:386` | add to the hard-coded property loop list |
| 5 | `runtime/cpp/core/node/animation.cpp:151` | `applyValue()` case: write `MorphStyle` + correct `PaintDirty`/`LayoutDirty` flags |
| 6 | `runtime/cpp/core/node/animation.cpp:228` | `restoreProp()` case: revert on finish (fill `none`) |
| 7 | `crates/morph-codegen/src/node_emitter.rs:2866` | `keyframe_style_value()`: IR field → `{KeyframeProperty::X, {…}}` |
| 8 | `crates/morph-codegen/src/node_emitter.rs:2895` | `raw_prop_to_enum()`: CSS name → enum (raw path, if `%`/transform-like) |
| 9 | `runtime/cpp/dev/ir_deserializer.h:122` | `keyframePropFor()` string→enum (dev-mode copy) + JSON push block |

Worked shape (numeric property, following `font-size`): one line in each map, one `case` per switch. No existing behavior is touched — purely additive, near-zero regression risk.

The maps have already drifted, which is why a single list is not sufficient today. `border-color`/`border-width` are in `is_animatable` and `keyframe_style_value` but absent from `raw_prop_to_enum` and dev `keyframePropFor` (each omission is individually defensible — baked-only values never take the raw path — but "which subset does this map cover?" is undocumented). Two live footguns follow from the duplication: `restoreProps()` iterates `p <= BorderGradient` (`animation.cpp:319`) so appending an enum member silently breaks revert-on-finish; and a property added to keyframes but not to the `style.cpp` transition functions animates in `@keyframes` yet snaps on `:hover` with no diagnostic.

## 7. What we are going to change (and not change)

### Phase 0 — safety without refactoring (~half a day, we do now)

1. **Consistency test (~1 hr).** One Rust test asserting `is_animatable` ⇔ `keyframe_style_value` ∪ `raw_prop_to_enum` cover the same set; plus a `--morph-self-test` check that dev `keyframePropFor` covers every `KeyframeProperty` enum member. Converts every "forgot a map" bug from silent-drop into a red test.
2. **`Count` sentinel (~5 min).** Add `Count` to `KeyframeProperty` and iterate `p < Count` in `restoreProps()` instead of the hard-coded `BorderGradient` bound.
3. **Warn on silent drop (~1–2 hrs).** `convert_keyframes` (`builder.rs:2916`) `continue`s without a word. Record dropped properties and surface a warning in `morph build` / `morph check` output. This is the compile-time diagnosis Chrome cannot do — use it.

### Phase 1 — conditional Rust-side table (only when triggered)

- **Trigger:** ≥3 properties added in one stretch, or an external contributor needs to add one. (Break-even on raw effort is ~15–20 properties; the trigger is about *routine-ness*, not count.)
- **Scope: Rust side only.** Collapse `is_animatable` + both codegen maps (+ serializer entry + feature mapping) into one table in `morph-ir`. Do **not** table-drive the C++ switches — heterogeneous style fields (floats, `float[4]`, `CssLength`, gradients, matrices) need per-type handling regardless, and `switch` is the idiomatic form. The Phase-0 test covers the C++ sync problem.
- Shape (in `morph-ir`, single source of truth): `css_name`, `ir_field`, `cpp_enum`, `feature`, `dirty` flags, `%` base, `raw_path`, `transition`, `compositor` — generating the Rust maps and feeding `morph check` diagnostics.

### Phase 2 — full timing-function support (ships with this change)

We decided to end the silent easing fallback as part of this work, because browsers and other frameworks now support the full set and authors reasonably expect `cubic-bezier()` and `steps()` to work. Today the runtime implements quadratic approximations of the five keyword easings and the builder drops `cubic-bezier()`/`steps()` without a word (`builder.rs:5456`). After this phase, the supported set is:

- **Keywords, spec-exact.** `linear` (identity) plus `ease`, `ease-in`, `ease-out`, `ease-in-out` evaluated as their CSS-specified beziers — `ease = cubic-bezier(0.25, 0.1, 0.25, 1.0)`, `ease-in = (0.42, 0, 1, 1)`, `ease-out = (0, 0, 0.58, 1)`, `ease-in-out = (0.42, 0, 0.58, 1)` — replacing the quadratic approximations. (Note `ease` today maps to `ease-in-out`; after this phase it uses its own curve.)
- **`cubic-bezier(x1, y1, x2, y2)`.** Full support including out-of-range `y` values (overshoot, e.g. `cubic-bezier(0.34, 1.56, 0.64, 1)`), which the interpolation code must tolerate by extrapolation rather than clamping.
- **`steps(n)` with jump terms.** `step-start` / `step-end` plus `jump-start`, `jump-end`, `jump-both`, `jump-none` — the whole family, since each is a few lines once the dispatcher exists.
- **Per-keyframe `animation-timing-function`.** CSS applies a keyframe block's timing function to the interval *starting* at that keyframe. Today the runtime applies one easing per animation; after this phase each keyframe carries its own easing and sampling uses the interval's curve. Documented-but-ignored becomes implemented.
- **`transition-timing-function` parity.** Whatever `animation-timing-function` accepts, transitions accept — one shared representation, no second subset to drift.

How we build it:

1. **One representation.** Normalize at build: keywords become their bezier parameters, so the runtime sees only `linear | cubic-bezier(x1,y1,x2,y2) | steps(n, jump)`. This extends `Easing` (today a 4-value enum, `render_frame.h:15`) into a tagged representation with parameters, carried on both `CssAnimation` and the compositor's `AnimationState` (whose `easing` field is currently a bare `uint8`).
2. **One solver, three call sites.** The cubic-bezier solver (Newton-Raphson with bisection fallback, the standard CSS approach) and the steps dispatcher live in a single shared header used by `updateCssAnimations` (`animation.cpp`), `interpolateStyles` callers (`style.cpp`), and the compositor loop (`compositor.cpp`) — so all three agree bit-for-bit. This is also what the future compositor-clock design reuses unchanged.
3. **Keep the build-time fallback for invalid input only.** Malformed `cubic-bezier()` (wrong arity, non-numbers) warns in `morph build` / `morph check` and falls back to the animation default — the Phase-0 warning machinery, now extended to easings.

Deferred, not dropped: `linear()` multi-stop easings (newer browser feature) stay a follow-up — see §12.

### Explicit non-goals

- **No engine merge** (see §5).
- **No WAAPI / imperative API** in this doc (belongs to the choreography design in `animations.md`: springs, enter/exit, shared elements).
- **No absolute-time compositor clock yet.** That is the real jank fix (compositor-owned clock + codegen-emitted `AnimationState` registration for compositor-safe animations, no per-frame main-thread round trip), but it changes threading contracts and needs its own design. Here we only record the direction we agree on.

### Build steps (when picked up)

1. Phase-0 test + sentinel + drop warning; run `cargo test --workspace`, fixture rebuilds, `./tests/runtime/run-selftests.sh`.
2. Phase-2 timing functions: representation + shared solver + per-keyframe easing + transition parity; easing fixtures for keywords (spec-exact values), `cubic-bezier()` (including overshoot), and each `steps()` jump term; `--morph-self-test` 0 failures.
3. On trigger: Rust-side property table + `morph check` diagnostics for non-compositor properties (nudge toward `transform`/`opacity`).
4. Separate future design: compositor-owned clock for statically classified compositor-safe animations.
5. Validation: `animation-test`, `animation-test-2.0`, `border-state` fixtures pixel-identical; `--morph-self-test` 0 failures.

## 8. Performance and binary-size impact

- **Phase 0 adds zero per-frame cost.** A build-time test, an enum sentinel (same iteration count), and a build-time warning (compile-time strings only). Runtime instruction count unchanged.
- **Phase 2 adds constant per-frame cost, no binary concern.** The cubic-bezier solver (~8 Newton/bisection iterations over a cubic) runs once per animated property per frame — microseconds against a 16ms budget — and the steps dispatcher is a closed-form jump. Keywords compile to their bezier parameters at build, so they cost exactly what `cubic-bezier()` costs. One shared solver function adds on the order of a hundred bytes of `.text`, inside the already-gated animation feature.
- **A property table adds zero runtime cost.** It generates the same switches/maps at compile time. No indirection, no virtual dispatch, no extra branches on the tick path.
- **Per-property runtime cost is pay-per-use and already gated.** Each new enum member adds a few bytes to `KeyframeValue` tables only for apps that use it; `#ifdef MORPH_FEATURE_*` keeps the feature define the elimination boundary (an app animating only `opacity` never links gradient/border/transform paths). Adding a property to `is_animatable` does not grow binaries that don't animate it.
- **Disk size:** negligible. One enum value + a handful of `case` arms ≈ tens of bytes of `.text`. The keyframe registry is per-app data, proportional to keyframes actually written.
- **Layout-animating properties (`width`/`height`/`left`/`top`/`font-size`/`border-width`) cost a main-thread layout per frame** — identical to browsers. This is physics, not implementation: changing a box invalidates dependents. Apps should prefer `transform`/`opacity` for 60fps; `morph check` nudges toward them (Phase 1).
- **Main-thread-per-frame cost for compositor-safe properties remains** until the compositor-clock design lands (§7): every animated frame still does style write → dirty → `commitFrame` → wait → draw. Fine for small trees; the documented limitation, not a regression from this plan.

## 9. Edge cases (locked behavior — do not regress)

- **Delay + fill:** `p < 0` shows first keyframe only under `Backwards`/`Both`; finished holds end state only under `Forwards`/`Both` (sample at `iterations - 1e-5`, so exact `1.0` doesn't wrap to the `0%` frame); otherwise revert to the pre-animation snapshot.
- **Fractional iterations** (`2.5`): stop mid-iteration, like browsers.
- **`reverse`/`alternate`/`alternate-reverse`:** parity flip per completed iteration; `alternate` starting direction is forward.
- **`paused`:** freezes the sample position (`elapsed` not advanced); resume continues, never jumps.
- **Hover swap:** animation list swaps instantly on `:hover` in/out while scalar fields still transition; removed animations revert the transform matrix instantly (browsers drop the effect; the transition never animates the matrix back).
- **Rapid retarget:** in-flight transitions snap to target before retargeting (prevents permanently frozen mid-interpolation styles under rapid clicks).
- **Reactive effects vs state transitions:** `interruptStateTransitions()` completes in-flight transitions before effects write, so effect values are never clobbered by stale snapshots.
- **`%` resolution:** against the parent box for `width`/`height`/`left`/`top`, own `min(w,h)` for `border-radius`, node's own box for transforms — resolved at tick time, so parent resizes mid-animation are picked up.
- **Transform `0deg→360deg`:** op-list interpolation preserves the wraparound; matrix fallback would collapse it to identity.
- **Incompatible gradients:** discrete flip at `f >= 0.5`, like browsers.
- **Duplicate offsets:** merged at parse (later block wins per property).
- **Missing property in a keyframe:** L2 semantics — interpolate when bounded on both sides, hold the last, keep underlying before the first. Never synthesize a value.
- **Invalid raw CSS at tick:** treated as no-op for that property that frame (underlying kept).
- **Comma-separated animation lists, `from`/`to`, grouped selectors (`0%,100%`), `infinite`, per-index longhand overrides:** all supported; nameless entries dropped.
- **Dev/prod parity:** dev JSON path (`ir_deserializer.h`) must rebuild the identical registry the prod codegen emits; the Phase-0 test enforces map coverage on both.
- **`cubic-bezier()`/`steps()`:** fully supported after Phase 2 (§7) — keywords are spec-exact, arbitrary beziers (including overshoot) interpolate by extrapolation, `steps()` covers all jump terms, and per-keyframe `animation-timing-function` applies to the interval starting at its keyframe. Only malformed easing syntax falls back (with a build warning).
- **Out of scope here (see `animations.md`):** exit+unmount ownership, shared-element mismatch fallback, `prefers-reduced-motion`, frozen-clock test hooks.

## 10. FAQ

**Why is property X not animatable?**
It is not in the animatable set (`builder.rs:4046`). Today it is silently dropped from keyframes — after Phase 0, `morph build` / `morph check` reports it. The cost of supporting it is one entry per map (§6); requests are evaluated against the Phase-1 trigger.

**Why do `width` animations jank while `transform` stays smooth?**
`width` changes layout (self + siblings + children) so every frame re-runs layout on the main thread — same in Chrome. Animating `transform: scaleX()` / `translateX()` instead avoids layout; `morph check` suggests this after Phase 1.

**Which timing functions are supported?**
After Phase 2 (§7): the five keywords with spec-exact curves, `cubic-bezier()` (including overshoot values), all `steps()` jump terms, per-keyframe `animation-timing-function`, and full `transition-timing-function` parity. Before Phase 2, only the keywords work (as quadratic approximations) and everything else silently falls back. `linear()` multi-stop easings are a deferred follow-up (§12).

**Why does `:hover` snap while `@keyframes` animates smoothly for the same property?**
The property is in the keyframe maps but missing from the transition functions (`interpolateStyles` / `applyStyleDelta`). That is a bug — it should be reported with the property name. The Phase-0 test is designed to catch exactly this class.

**Do animations keep running when app logic stalls the main thread?**
No — today the main thread authors every frame, so a stall freezes animations (they resume without jumping, since `elapsed` only advances by real `dt`). Chrome's composited animations survive this; Morph's will once the compositor-clock design lands. Frame work should be kept small.

**Does supporting more properties slow down apps or grow binaries?**
No. Unused properties compile out via feature defines; per-frame cost is proportional to animations actually running, not properties supported.

**Why aren't keyframes and transitions merged like in Chrome?**
We decided against it (§5): Chrome merged them because `element.animate()` (pause/seek/reverse/promises) forces one object model. Morph has no imperative animation API, so there is no forcing function — and merging would risk the transition unwind logic for no user-visible gain.

**Does Morph support springs / enter-exit / shared elements?**
Not in this doc — see `animations.md` (all ❌ Not built). Springs need velocity-state clocks, which compose naturally with the compositor-clock direction noted in §7.

**How are animations tested deterministically?**
Today: screenshot fixtures (`animation-test`, `animation-test-2.0`) plus `--morph-self-test`. Frozen-clock snapshots (`--freeze-animations`) are an open question in `animations.md` — easier for Morph than Chrome precisely because the animation set is closed at build.

## 11. Current state

| Piece | State |
|---|---|
| CSS `@keyframes` + `animation-*`, L2 sampling, easing, fill/direction | ✅ Shipped |
| `HoverTransition` + ancestor-hover interpolation | ✅ Shipped |
| Compositor vsync interpolation (fixed set) | ✅ Shipped (half-loop, §3.4) |
| Feature-gated dead-code elimination, baked keyframe values | ✅ Shipped |
| Consistency test across parallel property maps | ❌ Phase 0, step 1 |
| `Count` sentinel for `restoreProps()` bound | ❌ Phase 0, step 2 |
| Build warning on silently dropped keyframe properties | ❌ Phase 0, step 3 |
| Rust-side property table + `morph check` compositor nudges | ❌ Phase 1, on trigger (§7) |
| Spec-exact keyword easings + `cubic-bezier()` + `steps()` + per-keyframe easing + transition parity | ❌ Phase 2 (§7) |
| `linear()` multi-stop easings | ❌ Deferred follow-up (§12) |
| Compositor-owned absolute clock (jank independence) | ❌ Needs its own design |
| Engine merge / WAAPI | ❌ Explicit non-goal |

## 12. Open questions

- Should the drop warning be warn-by-default or error under `morph check --strict`? (Silent-compat vs strict-AOT philosophy.)
- Should non-interpolable properties animate discretely (spec-correct, Chrome behavior) instead of being dropped? Cheap (hold-until-50% like gradients already do) — needs a decision, not a design.
- Exact `morph check` diagnostic codes for animation lints (`mx-animation-*`?) and their docs pages under `docs/errors/`.
- `linear()` multi-stop easings: deferred until after Phase 2 lands. Needs a representation decision (stop list carried on `CssAnimation` vs normalized sampling at build) — a follow-up, not part of this change.
- Reduced-motion and frozen-clock hooks stay in `animations.md` — but the frozen clock is easier once classification is build-time; coordinate the two docs when either is picked up.

## 13. Appendix: Qt's animation approach — what we borrow, what we don't

### 13.1 How Qt handles animations

Qt has two animation systems. Both are property-based and object-driven — the inverse of Morph's static CSS.

**Qt Widgets: The Animation Framework (`QtCore`).** A class hierarchy of `QObject`s, all with `start()` / `stop()` / `pause()`, driven by a unified timer at absolute time per tick:

- `QAbstractAnimation` — clock, `duration`, `loopCount`, `direction`, `state`, `currentTime`.
- `QVariantAnimation` — interpolates a `QVariant` `startValue → endValue`, or `setKeyValueAt(step, value)` keyframes on 0..1, emitting `valueChanged()`. Built-in interpolation covers `Int` / `Double` / `Float` / `QPoint(F)` / `QSize(F)` / `QRect(F)` / `QLine(F)` / `QColor`. Other types (including custom ones) need `qRegisterAnimationInterpolator<T>(func)` or an `interpolated(from, to, progress)` override.
- `QPropertyAnimation` — binds to any `Q_PROPERTY` via `setTargetObject()` + `setPropertyName()`; each tick writes the interpolated value through the meta-object property system. Any declared property animates — there is no per-property allowlist.
- Groups: `QParallelAnimationGroup`, `QSequentialAnimationGroup` (plus `QPauseAnimation` for gaps), nestable like Morph's comma-separated animation lists.
- `QEasingCurve` — ~40 curves (`Linear`, `InQuad`/`OutQuad`, `InOutCubic`, `InElastic`, `OutBounce`, custom `BezierSpline`), spec-exact, per-animation.
- `QStateMachine` + `assignProperty()` with transition animations — Qt's answer to `:hover` / state transitions, reusing the same animation objects rather than a second engine.

**Qt Quick / QML: declarative animations.** Animation elements applied to properties, with three trigger styles: direct (`PropertyAnimation` / `NumberAnimation` / `ColorAnimation` / `RotationAnimation` with `target`, `property`, `to`, `duration`, then `.start()`); `Transition` on state change (`from` / `to`, wildcard `"*"`); and `Behavior on x { … }` — a default animation for *all future changes* of a property, which Morph has no equivalent of. Typed subclasses (e.g. `ColorAnimation`) exist for efficiency over generic `PropertyAnimation`. Beyond basics: `PauseAnimation`, `ScriptAction` / `PropertyAction`, physics types with real state (`SpringAnimation` with `mass` / `damping`, `SmoothedAnimation` tracking a moving target). `Animator` types (`OpacityAnimator`, `XAnimator`) run directly on the scene-graph / render thread, bypassing QML property writes — Qt's equivalent of compositor-driven animation.

The design point that matters: Qt interpolates by **value type** (`QVariant` metatype), not by property. A new animatable property costs one `Q_PROPERTY` declaration — zero engine changes.

### 13.2 Morph vs Qt

| | Qt | Morph |
|---|---|---|
| Paradigm | imperative objects (C++) / declarative elements (QML), created at runtime | static CSS text, compiled AOT |
| What is generic | everything: one engine keyed on value type | timing engine generic; property support is ~9 hand maps (§6) |
| New animatable property | declare `Q_PROPERTY` — works immediately | ~9 edits across Rust/C++ |
| Easing | 40+ exact curves + custom splines | after Phase 2: spec-exact keywords + `cubic-bezier()` + `steps()` — no 40-curve catalog, no custom spline (CSS has no spelling for either) |
| State transitions | same animation objects inside `Transition` / `QStateMachine` | separate `HoverTransition` engine — deliberate, see §5 |
| Off-thread animation | `Animator` on the scene-graph thread | compositor half-loop today → compositor clock (future design) |
| Springs / physics | built-in (`SpringAnimation`) | not built (`animations.md` future) |
| Binary cost | tens of MB (`QtCore` + `QtGui` + `QtQuick`) | 162KB hello-world; animation code gated out when unused |

### 13.3 Why we don't adopt Qt's approach wholesale

Qt's mechanisms solve a problem Morph doesn't have, at a cost Morph can't afford:

- **`QObject` + meta-object property system** (moc, dynamic `setProperty`, string lookups, per-object overhead). Morph compiles `MorphStyle` structs to straight C++ fields with build-baked values — a meta-object layer would undo that advantage on every style access, not just animations.
- **`QVariant` interpolation** (type-erased values, virtual `interpolated()` dispatch per frame per property). Morph interpolates raw `float` / `float[4]` in a tight loop — the same arithmetic with no boxing and no dispatch.
- **Runtime animation objects** (heap allocation, lifetime management, an always-shipped engine). Incompatible with `#ifdef MORPH_FEATURE_*` dead-code elimination: Morph apps without animations ship zero animation code; Qt ships the machinery to everyone.
- **QML is interpreted; user properties are declared at runtime.** Qt cannot know anything at build, so everything must be late-bound. Morph knows the full animation set at `morph build` — copying Qt's runtime machinery would throw away Morph's only structural advantage.

Our decision as a team: we follow Qt's **principle** (generic over value types, one engine) but keep Morph's **mechanism** (static, baked, gated). Phase 1's value-kind table (§7) is that principle adapted to AOT — classify each property by value kind (scalar / color / length / gradient / matrix) and generate the cases at compile time instead of resolving them through a `QVariant`-style runtime.

### 13.4 What we take, what we don't

Take:

1. **Value-type genericity → feeds the Phase-1 table.** The single most transferable idea: the per-property switches in `resolveValue` / `applyValue` / `restoreProp` are per-*type* logic wearing per-property clothes. The table's `value-kind` dimension is Qt's `QVariant`-metatype dimension, evaluated at build instead of at runtime.
2. **`Behavior`-style default animations → candidate proposal for `animations.md`.** QML's `Behavior on x` (animate all future changes of a property) fits Morph's reactive `morphState` model — where style changes come from state, not pseudo-classes — better than CSS `transition` alone. It needs its own design (interaction with the `HoverTransition` unwind in `style.cpp:397-460` and with `interruptStateTransitions`), so it belongs to the choreography scope, not this doc.

Don't take:

- **`QStateMachine`.** Morph's hover/active model is simpler and sufficient; a state machine adds states, events, and history semantics nobody asked for.
- **Runtime animation groups.** Comma-separated `animation` lists plus `SequentialAnimation`-equivalent ordering already cover composition declaratively; heap-allocated group objects buy nothing here.
- **`Animator` as a second element type.** Build-time compositor classification (§4, Phase 1) routes transform/opacity off-thread without a parallel API — one spelling, two paths, compiler's choice.
- **The 40-curve catalog.** CSS defines keywords + `cubic-bezier()` + `steps()`; `cubic-bezier()` with out-of-range `y` already covers overshoot/elastic-ish curves. A custom-spline easing has no CSS spelling, so there is nothing to parse — Phase 2 is the complete set for this styling model.

### 13.5 Qt FAQs

**Could Morph animate any style field automatically, like any `Q_PROPERTY`?**
Only after Phase 1's value-kind table exists — and even then only for fields whose value kind the table covers. Until then, §6's maps are the mechanism. The difference from Qt is *when* the genericity resolves (build vs runtime), not whether it exists.

**Should Morph add `SpringAnimation`?**
Belongs to `animations.md`, not this doc. Springs need velocity-state clocks, which compose with the compositor-clock direction in §7 — the same way Qt's `SpringAnimation` composes with its scene-graph thread.

**Should Morph add `Behavior on x`?**
Candidate for `animations.md`. Open design questions: precedence between a behavior animation and a `:hover` transition on the same property, and whether reactive-effect writes retrigger behaviors (the `interruptStateTransitions` contract in `style.cpp:595` would need a behavior clause).

**Why doesn't Morph match `QEasingCurve`'s 40 curves?**
There is no CSS spelling for them. `cubic-bezier()` already expresses any single-segment curve — including `OutBack`-style overshoot via `y > 1` — and `steps()` covers holds. Matching the catalog would mean inventing non-CSS syntax, which contradicts the "plain CSS" styling model.

**Does Qt's `Animator` map to Morph's compositor path?**
Conceptually, yes: interpolation owned by the rendering thread, bypassing main-thread property writes. The difference is routing — Qt asks the author to pick `Animator` vs `Animation` per use; Morph classifies per property at build (Phase 1) so authors write one spelling. Same destination, no second API to learn.

**Isn't per-property code in Morph just tech debt Qt avoided?**
Partly, yes — §6 documents the drift. But the debt is bounded (~14 properties, closed set) and Phase 0's test converts the failure mode from silent to loud. Qt paid differently: a meta-object compiler, a type-erased value system, and megabytes of runtime carried by every app. For a closed property set compiled AOT, our trade is cheaper.

## 14. References

- Code: §3.5 table (authoritative paths + lines).
- User docs: `docs/css/animations.md`, `docs/css/transitions.md`.
- Design: `docs/future/css/animations.md` (choreography: springs, enter/exit, shared elements), `docs/future/rendering/performance.md`, `docs/future/rendering/forge-tile-pool.md`.
- Internals: `help/compositing-thread.md` (thread model, promotion rules, Morph-vs-Chrome table), `help/architecture.md` (Flash/Forge, flush order).
- Chrome: Blink `third_party/blink/renderer/core/animation/README.md` (Web Animations timing model); `third_party/blink/renderer/core/css/css_properties.json5` (`interpolable` flag); `CompositorAnimations` allowlist + fallback-reason tables; `core/animation/README.md` timeline section.
- Qt: `QVariantAnimation` / `QPropertyAnimation` / `QAbstractAnimation` class docs (`doc.qt.io/qt-6/qvariantanimation.html`); *Animation and Transitions in Qt Quick* (`doc.qt.io/qt-6/qtquick-statesanimations-animations/`); `QEasingCurve` type list; *The Animation Framework* overview (`animation-overview.html`).
