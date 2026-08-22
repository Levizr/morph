# Bug Report — August 23, 2026

> **AI-generated report.** This document was produced by an AI code-audit pass over the source tree. It may contain findings that are not actually there — misread code paths, wrong line numbers, or intended behavior mistaken for bugs. **Every item needs manual verification before fixing.** Treat each entry as a *lead*, not a confirmed defect.
>
> Scope: three parallel audits — dev runtime/DevTools/renderers (`runtime/dev/`, `runtime/renderers/`), layout & text engine (`runtime/core/node/layout.cpp`, `runtime/ui/`), and the Python compiler pipeline (`morph/parser/`, `morph/js/`, `morph/style/`, `morph/dev/`, `morph/cli/`).

---

## 1. Dev runtime & DevTools (`runtime/dev/`)

### 1.1 Buffered socket messages starved — intermittent missed hot reloads
`runtime/dev/dev_socket.cpp:128-148` (with `main.cpp:473`)
`readMessage()` calls `select()` before checking whether a complete message already sits in `m_recvBuf`. If two messages coalesce into one segment (e.g. `__log__` + IR), the second stays stranded; the main loop's zero-timeout `select` exits and the saved-change IR isn't applied until another message arrives.
*Symptom:* edits appear only after the next save/event, or apply late.

### 1.2 Use-after-free of focused/captured node after hot reload
`runtime/dev/main.cpp:504-512`; deref sites `runtime/core/window.cpp:190-193, 203-204, 117-118`
The tree-swap block clears hovered/selected nodes but never resets `MorphNode::s_focusedNode` or `s_mouseCapture`. Key/char callbacks can then hit freed memory if an input had focus during a reload.
*Symptom:* crash or garbage behavior typing right after saving a file.

### 1.3 DevTools width not clamped to window — panel hides app content
`main.cpp:175` (F12), `main.cpp:243-245` (drag clamp order), `window.h:99-102`, `window.cpp:284-296`
F12 applies the default 320px panel without comparing to window width; nothing re-clamps on resize; three inconsistent constants (120px content floor / 240px panel min / 360px drag reserve). In `cursorCb` the min-clamp runs before the max-clamp, so narrow windows can end up with a panel narrower than its minimum (or ≤0).
*Symptom:* on windows < ~panel+120px, app content is covered/mislaid out; divider dragging produces squished/vanishing panels.

### 1.4 Forge damage rects ignore ancestor scroll offset
`renderers/forge/forge.cpp:151-162, 187-197`, `damage.cpp:99-112`; compare `window.cpp:580` vs correct `screenY = sy - scrollOffset` at `:525`
Damage recorded at root-space coords excluding ancestor `scrollY`; renderNode's damage-intersection uses the unscrolled box too.
*Symptom:* ghosting/stale bands at edges of scrolled lists in forge mode.

### 1.5 Forge `g_prevRects` survives hot reload → blank widgets
`renderers/forge/forge.cpp:164-215`
Prev-rect map keyed by raw `MorphNode*`, cleared only on node-count change/fullscreen. After a same-node-count tree swap, recycled addresses hit stale entries; paint dirt set by `syncPaintDirtyTree` gets cleared at lines 200-201, so display lists are never recorded.
*Symptom:* widgets render blank/stale after hot reload until geometry changes.

### 1.6 Conditional branches leak every reload
`runtime/dev/ir_deserializer.h:512-521`, `main.cpp:505-507`
Both `then_nodes`/`else_nodes` are deserialized + registered but only the active branch is attached; `deleteNodeTree` can't reach the inactive branch and the registry swap drops the last reference.
*Symptom:* unbounded memory growth per save in apps using conditionals.

### 1.7 Log ring buffer mutexes don't guard anything
`runtime/dev/dev_log.h:44-70`
`devLogAdd`, `devLogClear`, `devLogSnapshot` each declare their own function-local `static std::mutex` — three different mutexes over one deque (contrast shared `devNetMutex()` in `dev_net.h:28-31`). Latent today because callers are main-thread.
*Symptom:* race if any worker thread logs concurrently with snapshot.

### 1.8 Outer scroll container swallows wheel events
`runtime/core/node/events.cpp:104-118`
Scrollable node checks itself before children and returns true whenever the point is in bounds — even when already at scroll limit. Nested scrollers never receive wheel input; no scroll chaining.
*Symptom:* inner list won't wheel-scroll inside an outer scroller.

### 1.9 Cross-thread signal access unsynchronized
`reactivity/signal.h:65-77`, `net.cpp:43-71`, `effect.cpp:72`
`Signal<T>::get()` reads `value_` without the mutex once subscribed while `fetch()`'s worker writes via `set()` and resumes coroutines off-main-thread; `run_pending_effects()` checks `s_pending.empty()` without its mutex.
*Symptom:* torn string reads, rare lost effect wakeups in fetch-using apps.

### 1.10 Effect nodes accumulate across hot reloads
`reactivity/effect.cpp:25-34, 77-86`; driven by `main.cpp:130-146`
Cleanup unsubscribes but never deletes `EffectNode`s or clears `s_pool`; `destroy_all_effects()` has no callers. Each reload leaks the effect set plus captured closures.
*Symptom:* slow memory growth proportional to reload count.

### 1.11 Inspect mode hit-tests through the open panel
`runtime/dev/inspector.h:167-173`
`updateHover` has no exclusion for `mx >= winW - m_panelW`.
*Symptom:* elements hidden under the panel highlight/select while cursor is over it.

### 1.12 Scrollbar NaN on very short windows
`runtime/dev/inspector.h:129-131, 141-152`
Thumb floored at 24px but `viewH` can be smaller; `(viewH - thumbH) ≤ 0` yields NaN/negative values that survive clamps (NaN comparisons are false).
*Symptom:* logs/network scrollbar jumps or vanishes on short windows.

---

## 2. Layout & text engine (`runtime/core/node/`, `runtime/ui/`)

### 2.1 Stale line total shifts grow rows off-center
`layout.cpp:418` vs `:462`
Grow/shrink mutates item main sizes but never updates `line.totalMain`; placement recomputes stale free space.
*Symptom:* `flex-grow > 0` + `justify-content: center/flex-end/space-between` shifts the row by the "vanished" space.

### 2.2 Flex margins added twice
`layout.cpp:512-513` + `:241-242`
Placement pre-adds leading margin; `MorphNode::layout` adds it again.
*Symptom:* every flex child sits at double left/top margin, drifting cumulatively along the line.

### 2.3 Wrap test omits gap
`layout.cpp:405` vs `:410`
Wrap check ignores inter-item `gap` that accounting later adds to `totalMain`.
*Symptom:* lines overflow by exactly one gap; shrink squeezes instead of wrapping.

### 2.4 Column cross-size double-subtracts margins
`layout.cpp:527, 530-544, 550` (+ relayout `w = parentW - ml - mr` at `:220`)
Pre-layout width for non-stretch column children excludes margins; relayout subtracts them again.
*Symptom:* `align-items: center/flex-end` children with horizontal margins render ~2×margin narrower — early wrap/clipped labels.

### 2.5 max-width clamped after auto-margin centering
`layout.cpp:224-226` → `:241` → clamp at `:282`
Auto margins resolve against pre-clamp size; `max-width` clamps afterwards without recomputing margins/x (min-width grows similarly).
*Symptom:* cards with `max-width` + `margin: 0 auto` sit flush toward their stale position instead of centered.

### 2.6 Text ignores own padding/border when wrapping/drawing
`runtime/ui/text.h:94-96, 213-219`
`wrapW = w` unreduced by `pl+pr+borders`; lines drawn from `lx = x`.
*Symptom:* first glyph in the padding gutter; overflow past right edge by pl+pr.

### 2.7 Font inheritance single-level + default sentinel collision
`runtime/ui/text.h:265-277`; no propagation in `ir/builder.py`
Effective font props fall back to direct parent only, gated on value equaling defaults (16/normal/left); explicit `font-size:16px` indistinguishable from unset.
*Symptom:* explicit 16 inside 24 parent renders 24; grandparent's 32 never reaches grandchildren through unstyled divs.

### 2.8 Emoji measured with text-font advances, drawn with emoji-font advances
`render/gl_renderer.cpp:530-545` vs `:623-657` (atlas routing `:373-385`)
`measureTextWidth` shapes against the text atlas only; `drawText` routes non-ASCII to emoji atlas with different advances.
*Symptom:* centered/right-aligned lines with emoji sit visibly off-position; wrong wrap breaks.

### 2.9 Input caret X drifts against kerned rendering
`runtime/ui/input.h:503-504, 520-521`
Caret/selection edges are standalone prefix measurements; field drawn as one HarfBuzz run with cross-boundary kerning.
*Symptom:* caret drifts off glyph boundary around kerning pairs ("Ta", "AV").

### 2.10 Flex contentWidth counts hidden/whitespace children
`runtime/core/node/node.cpp:110-123`
Flex branch sums all children incl. `display:none` (block branch skips at :145-147) and whitespace-only nodes dropped elsewhere (`layout.cpp:380`).
*Symptom:* shrink-to-fit flex rows wider than visible content → phantom trailing space, off-center containers.

### 2.11 Button centering shifts only direct children
`layout.cpp:936-946`
Post-layout vertical-centering offset applied to qualifying direct children only; grandchildren keep pre-shift coordinates baked into display lists.
*Symptom:* `<button><div>label</div></button>` fixed-height — inner div background moves, label stays; label detaches from pill.

---

## 3. Python compiler pipeline (`morph/`)

### 3.1 Save during in-flight reload silently dropped
`cli/cmd_dev.py:150-151`
Callback returns when `_reloading` is true; watcher fires debounced batches exactly once, nothing reschedules.
*Symptom:* last edit never compiled until next save.

### 3.2 Multi-file atomic saves lose compiles
`dev/watcher.py:41-50`
Debounce keeps only latest pending path; coalesced-away paths stay stale (content-hash cache skips unrelated work).
*Symptom:* some files of a multi-file save stay old until touched again.

### 3.3 Later CSS import clobbers earlier same-selector rules
`dev/pipeline.py:129-147` (merge via `css_rules.update(rules)`)
Per-sheet merging is per-property, but import merging replaces whole selector entries.
*Symptom:* `import "./a.css"; import "./b.css"` both styling `.card` — a.css declarations silently dropped.

### 3.4 HTML width/height attributes beat CSS/Tailwind
`ir/builder.py:746-752`
Attributes applied after stylesheet rules and Tailwind (comment claims priority 4). Browsers treat attributes as lowest-priority presentational hints.
*Symptom:* `<img class="w-full" width="400">` renders fixed 400px; responsive images break.

### 3.5 Sibling combinators match as descendants; specificity gaps
`style/selector.py:27-35, ~83`
`h2 + p` falls into descendant branch matching any p under h2 ancestor; specificity ignores pseudo-classes; grouped selectors take max specificity of alternatives rather than the matching one.
*Symptom:* rules style descendants they shouldn't; cascade ties resolved wrongly.

### 3.6 Mismatched-quote string literals emit invalid C++
`js/codegen.py:1042` + `js/ast_builder.py:712-715`
Raw interior kept (`strip("'\"")`) and emitted with zero escaping.
*Symptom:* `const msg = 'say "hi"';` generates `"say "hi""` — logic `.so` fails to compile.

### 3.7 Multi-line template literals embed raw newlines
`js/codegen.py:1056-1086` (`_extract_template_parts` escapes only `\` and `"`)
*Symptom:* backtick literals spanning lines produce C++ string literals with literal newlines — compile error on reload.

### 3.8 Arrow capture heuristic unsound
`js/codegen.py:574-592` (+ `:494-500`)
`[&]` iff `_fn_body_depth > 0`, but counter incremented only by function declarations: returned arrow captures stack local by reference (UB); top-level arrows get `[]` so plain-local handlers fail to compile.
*Symptom:* `makeCounter` pattern UB; event handlers referencing non-state locals don't compile.

### 3.9 Nested callbacks in handlers discard return values
`js/codegen.py:251-260` (via `:574+`)
`event_handler=True/_drop_return_value` propagates into nested arrows; `.map(x => x.id)` inside onClick becomes void lambda.
*Symptom:* mapped data becomes undefined inside handlers.

### 3.10 Member-expression JSX tags truncated
`parser/jsx_walker.py:651-658`
`<Foo.Bar />` resolves to tag `"Foo"` (first identifier child only) — wrong component symbol instead of error/resolution.
*Symptom:* namespaced components resolve incorrectly.

---

## Triage suggestion

Highest user impact first: **3.1/3.2** (lost edits), **2.2** (every flex layout), **1.3** (reported DevTools symptom), **1.1** (reported missed reloads), **2.1/2.5** (centering), **3.3/3.4** (wrong styles), **3.7** (common code fails reload), **1.4/1.5** (forge artifacts).

Again: **verify before fixing** — this report was AI-generated and may contain errors.
