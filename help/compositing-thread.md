# Compositor Thread Architecture

## Goal

Decouple animation and rendering from the main thread so that:
- Animations run at vsync rate (60/120/144Hz) regardless of main-thread layout/paint load
- GL context is owned exclusively by one thread (no context contention)
- Main thread handles events, style, layout, and paint — producing a lock-free snapshot for the compositor

---

## Thread Model

```
MAIN THREAD                          COMPOSITOR THREAD
(events + style + layout + paint)    (vsync + GL + animation interpolation)
                                       owns GL context exclusively
┌──────────────────────────────┐     ┌──────────────────────────────────┐
│                              │     │                                  │
│  glfwPollEvents()            │     │  WaitForVsync()                  │
│    → push events to SPSC Q  │     │  (glfwSwapInterval(1) causes     │
│                              │     │   glfwSwapBuffers to block)      │
│  Drain event queue           │     │                                  │
│  → style / hover / rules     │     │  Atomic load front RenderFrame   │
│                              │     │  (acquire)                       │
│  layoutIfNeeded()            │     │                                  │
│  (only dirty nodes)          │     │  Interpolate compositor anims    │
│                              │     │  t = (now - startTime) / dur     │
│  recordDisplayList()         │     │  → write animOffsetX/Y/opacity   │
│  (paint-dirty nodes)         │     │    onto FlatRenderNode           │
│                              │     │                                  │
│  Flatten tree → RenderFrame  │     │  Execute display lists           │
│                              │     │  (all GL draw calls)             │
│  Atomic swap:                │     │                                  │
│  g_frontFrame.store(back,    │     │  Process upload queue            │
│                       release)│     │  (glTexImage2D for new textures) │
│                              │     │                                  │
│  Drain feedback queue        │     │  Push completion events to       │
│  (anim completion callbacks) │     │  feedback SPSC queue             │
│                              │     │                                  │
│  Yield until next event      │     │  glfwSwapBuffers ← blocks here   │
│  or pending render           │     │  until next vsync                │
│                              │     │                                  │
└──────────────────────────────┘     └──────────────────────────────────┘
```

**Key rule:** GL context is created on the main thread, then handed to the compositor thread exactly once. The main thread never touches GL again.

---

## Data Structures

### RenderFrame — the lock-free shared snapshot

```cpp
struct FlatRenderNode {
    int id;
    float x, y, w, h;             // layout-computed base position
    float bgColor[4], color[4];
    float borderRadius;
    float borderWidth, borderColor[4];
    // ... other renderable style fields

    // Compositor-written each vsync tick:
    float animOffsetX = 0, animOffsetY = 0;  // from X/Y/transform anims
    float opacity = 1.0f;                     // from opacity animation

    // Display list (pre-recorded paint commands)
    int dlOffset;    // index into frame's flat drawOps array
    int dlCount;

    // Tree structure (flat indices, no pointers)
    int parentId;
    std::vector<int> children;
};

struct RenderFrame {
    std::vector<FlatRenderNode> nodes;
    std::vector<DrawOp> drawOps;
    std::vector<AnimationState> animations;
    uint64_t frameId;
    double timestamp;
};

// Double-buffered (two back buffers for ping-pong):
RenderFrame g_backFrames[2];          // main thread writes
std::atomic<RenderFrame*> g_frontFrame;  // compositor reads
int g_backIndex = 0;                  // main thread toggles
```

### AnimationState

```cpp
struct AnimationState {
    int nodeId;
    AnimProperty prop;         // only compositor-safe properties
    float from, to;
    double startTime;          // absolute time (steady_clock)
    float duration;
    Easing easing;
    bool running;
};
```

### SPSC Event Queues (lock-free)

```cpp
// GLFW callbacks → Main thread
SPSCQueue<MorphEvent> g_eventQueue;

// Compositor → Main thread (animation completion, etc.)
SPSCQueue<AnimCompletionEvent> g_feedbackQueue;

struct AnimCompletionEvent {
    int nodeId;
    AnimProperty prop;
};
```

---

## Thread Responsibilities

### Main Thread

| Responsibility | Details |
|---|---|
| GLFW event processing | `glfwPollEvents()` — callbacks push into `g_eventQueue` |
| Style computation | `applyStyleDelta()`, hover rules, ancestor hover |
| Layout | `layoutIfNeeded()` — box model, flex, inline |
| Display list recording | `recordDisplayList()` — writes `DrawOp`s into node's display list |
| Tree flattening | Walks `MorphNode` tree → fills `g_backFrames[g_backIndex]` |
| Frame commit | `g_frontFrame.store(&g_backFrames[g_backIndex], release)` |
| Feedback drain | Polls `g_feedbackQueue` for animation completion → fires callbacks, cleans up `AnimationState` |
| Texture pixel generation | Decodes images, generates pixel buffers → pushes to upload queue |
| JS/DOM mutation | All tree/style changes go through main thread only |

### Compositor Thread

| Responsibility | Details |
|---|---|
| GL context owner | `glfwMakeContextCurrent()` at thread start — exclusive owner |
| Frame acquisition | `g_frontFrame.load(acquire)` each vsync tick |
| Animation interpolation | `t = (now - anim.startTime) / anim.duration` → sets `animOffsetX/Y`, `opacity` on `FlatRenderNode` |
| Display list execution | Iterates `FlatRenderNode`s, issues GL draw calls at `(x + animOffsetX, y + animOffsetY)` |
| Texture upload | Drains upload queue → `glTexImage2D()` |
| Compositor feedback | Pushes `AnimCompletionEvent` to `g_feedbackQueue` when animation finishes |
| Buffer swap | `glfwSwapBuffers()` — blocks until next vsync |

---

## Property Promotion Rules

| Property | Compositor-animatable? | Why |
|---|---|---|
| `transform: translateX/Y` | **Yes** | Just offsets `animOffsetX/Y`; no layout impact |
| `opacity` | **Yes** | Shader uniform / blend factor; no layout impact |
| `background-color` | **Yes** | Uniform change; re-record display list on main thread, then compositor just reads new color |
| `color` | **Yes** | Same as background-color |
| `border-radius` | **Yes** | Geometry change; re-record display list on main thread |
| `x`, `y` (position) | **Yes** | Treated as transform translate; layout computes base, compositor offsets |
| `width`, `height` | **No** | Affects layout of self AND children; must go through main-thread layout pass |
| `padding`, `margin` | **No** | Affects layout of self and siblings |
| `font-size` | **No** | Affects text layout, line height, wrapping |
| `border-width` | **No** | Affects box model sizing |

**Rule of thumb:** If changing the property can alter the position or size of other nodes, it must go through main-thread layout. If it only affects the visual appearance of the node itself, it can be compositor-animated via display-list re-recording.

---

## Animation Flow

### Starting an animation

```
User/JS code: element.startAnimation(X, 500, 1.0, EaseOut)
    │
    ▼
Main thread: MorphNode::startAnimation()
    │  Stores {nodeId, X, from=currentX, to=500, startTime=now, duration=1.0, running=true}
    │  into pending AnimationState
    ▼
On next flatten: included in RenderFrame.animations
    │
    ▼
Atomic swap → compositor picks it up
```

### Per-vsync composition

```
Compositor tick:
    │
    ├─ Load front RenderFrame
    │
    ├─ For each AnimationState:
    │     t = (now - startTime) / duration
    │     if t >= 1.0:  t = 1.0, mark finished
    │     val = from + (to - from) * easing(t)
    │     if prop == X:  node.animOffsetX = val - node.x
    │     if prop == Y:  node.animOffsetY = val - node.y
    │     if prop == Opacity:  node.opacity = val
    │
    ├─ For each FlatRenderNode:
    │     drawX = node.x + node.animOffsetX
    │     drawY = node.y + node.animOffsetY
    │     executeDisplayList(node, drawX, drawY, node.opacity)
    │
    ├─ If any animation finished:
    │     push AnimCompletionEvent to g_feedbackQueue
    │
    └─ glfwSwapBuffers()  ← blocks for vsync
```

### Animation completion (feedback loop)

```
Compositor: detects t >= 1.0
    │
    ├─ Pushes {nodeId, X} to g_feedbackQueue
    │
    ▼ (next main-thread tick)

Main thread: drains g_feedbackQueue
    │
    ├─ Fires onAnimationEnd callback
    ├─ Removes AnimationState from pending list
    └─ Stops including it in next RenderFrame
```

---

## Texture Upload

**Critical rule:** All GL calls happen on compositor thread only. No `glGenTextures` or `glTexImage2D` on main thread.

```
Main thread:
    │
    ├─ loadTexture("image.png")
    ├─ Decodes PNG via stb_image → raw RGBA pixels
    ├─ Pushes { path, pixels, width, height } into upload_queue
    └─ Returns placeholder texture ID immediately (or defers until upload complete)

Compositor thread (each tick, before drawing):
    │
    ├─ Drains upload_queue
    ├─ glGenTextures(1, &tex)
    ├─ glBindTexture(GL_TEXTURE_2D, tex)
    ├─ glTexImage2D(..., pixels)
    ├─ glGenerateMipmap(GL_TEXTURE_2D)
    ├─ free(pixels)
    └─ Stores { path → tex } mapping for use in display list execution
```

The upload queue is another lock-free SPSC queue (or a simple mutex-guarded deque since uploads are infrequent and not on the hot path).

---

## Event Handling

GLFW callbacks run on the main thread (during `glfwPollEvents()`):

```
GLFW mouse callback
    │
    ├─ Creates MorphEvent {MouseMove, x, y}
    └─ g_eventQueue.push(event)

Main thread (next tick):
    │
    ├─ while (auto e = g_eventQueue.pop())
    │     processEvent(e)
    │     → hitTest, hover tracking, dispatchEvent
    │     → style changes → markDirty
    │
    └─ After drain: layoutIfNeeded() + recordDisplayList() + flatten + swap
```

---

## JS/DOM Manipulation

All tree mutations go through the main thread:

```
JS: el.style.width = "200px"
         │
         ▼
    Main thread: marks node LayoutDirty
         │
         ▼ (next frame)
    Main thread: layoutIfNeeded() → re-layouts
         │
         ▼
    recordDisplayList() → flatten → atomic swap
         │
         ▼ (next vsync)
    Compositor: picks up new frame with updated positions
```

**Latency:** One frame for layout-affecting changes (same as browsers). For compositor-safe properties (transform, opacity), JS writes to main-thread state → main thread marks dirty → next flatten includes updated AnimationState → compositor interpolates.

---

## Synchronization Strategy

**Zero mutexes on the hot path.** All cross-thread communication is via lock-free atomics and SPSC queues:

| Mechanism | What it protects |
|---|---|
| `std::atomic<RenderFrame*> g_frontFrame` | The shared frame pointer (release/acquire semantics) |
| `SPSCQueue<MorphEvent> g_eventQueue` | GLFW events → main thread |
| `SPSCQueue<AnimCompletionEvent> g_feedbackQueue` | Animation completion → main thread |
| `SPSCQueue<UploadJob> g_uploadQueue` | Texture pixel data → compositor thread |
| `std::atomic<bool> g_pendingFrame` | Signal compositor that new frame is ready |

**No shared mutable state.** Once `g_frontFrame` is swapped, the main thread no longer touches that `RenderFrame` instance (it writes to the other back buffer instead).

---

## Implementation Order

| Phase | What | Files |
|---|---|---|
| **1** | Define `FlatRenderNode`, `RenderFrame`, `AnimationState`, SPSC queue | `render_frame.h`, `spsc_queue.h` |
| **2** | Add `flatten()` to `MorphNode` that produces `FlatRenderNode` array | `node.h`, `node.cpp` |
| **3** | Create `Compositor` class — thread, vsync loop, frame load | `compositor.h`, `compositor.cpp` |
| **4** | Move GL context ownership to compositor, rewrite `window.cpp` | `window.h`, `window.cpp` |
| **5** | Move animation interpolation into compositor | `compositor.cpp` |
| **6** | Add feedback queue for completion events | `compositor.cpp`, `node.cpp` |
| **7** | Add upload queue for texture pixel data | `compositor.cpp`, `gl_renderer.cpp` |
| **8** | Update widget `recordDisplayList`/`executeDisplayList` to work on flat nodes | `morph_rect.h`, `morph_text.h`, etc. |
| **9** | Rewrite main loop — events → layout → paint → commit → yield | `main.cpp` |

---

## Comparison: Morph vs Chrome

| Aspect | Chrome | Morph (after this change) |
|---|---|---|
| **Main thread** | Blink: style, layout, paint, JS | Events, style, layout, paint, tree flatten |
| **Compositor thread** | cc: animations, transforms, scroll, tile compositing | Anim interpolation, display list exec, GL calls |
| **GPU process** | Viz: isolated process for GL | Single compositor thread (simpler) |
| **Animation on compositor** | transform, opacity, filter, scroll | X/Y offset, opacity (extensible) |
| **Layout-triggering anims** | width/height → main-thread re-layout | width/height → main-thread re-layout |
| **Texture upload** | GPU process via shared memory | Upload queue → compositor does glTexImage2D |
| **Frame timing** | VSync-aligned (Chrome's BeginFrame) | VSync-aligned via glfwSwapInterval(1) |
| **Cross-thread sync** | Scheduler commands, copy-on-write tree | Atomic frame swap, SPSC queues |
