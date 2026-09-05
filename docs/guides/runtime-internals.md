# Runtime Internals Deep Dive

Internal architecture of the Morph C++ runtime for contributors and advanced users.

## Core Architecture

```
runtime/cpp/
├── core/                    # Scene graph + windowing foundation
│   ├── node.h              # MorphNode, DirtyFlag, HoverTransition, flatten()
│   ├── window.h            # GLFW window, event loop, renderer dispatch
│   ├── window_manager.h    # Multi-window (stub)
│   ├── compositor.h        # Compositor thread, SPSC queue, vsync interp
│   ├── render_frame.h      # Lock-free frame data for compositor
│   └── draw_op.h           # DrawOp enum (Rect, Text, Image, Border, Clip, Scissor)
├── render/                 # Shared GL primitives (both backends use)
│   ├── gl_renderer.h       # Batched rects, text, borders, clips, textures
│   └── shader.h            # SDF text, rounded rect, image shaders
├── renderers/              # Paint backends
│   ├── renderer.h          # RenderMode {Flash, Forge}, activeRenderMode()
│   ├── flash/flash.h       # Full clear + replay (default, ~22 MB @1080p)
│   └── forge/              # Retained FBO + DamageSet (beta)
├── style/                  # CSS → GPU pipeline
│   ├── style.h             # MorphStyle + feature-gated mixins
│   └── features/           # flex, position, scroll, border, cursor, zindex,
│                           # opacity, transform, animation, outline, shadow
├── ui/                     # Concrete widgets
│   ├── rect.h, text.h, button.h, input.h, image.h
│   ├── radius.h, viewport_node.h, viewport_driver.h, morph_list.h
├── widgets/                # Thin wrappers
│   └── morph_rect.h, morph_text.h, morph_button.h, morph_image.h, morph_radius.h
├── net/                    # Networking
│   └── net.h               # Headers, Response, fetch(), HttpAwaitable
├── reactivity/             # Async + signals
│   ├── signal.h            # Signal<T>, create_effect, create_memo
│   ├── promise.h           # morph::Result<T> (Promise<T>), ValueAwaiter
│   └── task.h              # morph::Task (Promise<void>), next_frame, timers
├── types/                  # JS value types
│   ├── js_types.h          # Umbrella + js_value_format.h (MORPH_NO_FORMAT)
│   ├── js_value.h          # JsValue variant + JS semantics
│   ├── js_string.h         # JsString + methods
│   ├── js_number.h         # JsNumber (int64/double/bigint) + ops
│   ├── js_boolean.h
│   ├── js_array.h          # shared_ptr<vector<JsValue>>
│   └── js_object.h         # shared_ptr<map<string,JsValue>>
└── dev/                    # DevTools (compiled out in build)
    └── inspector.h, dev_log.h, dev_net.h, dev_socket.h, ...
```

## Scene Graph (`core/node.h`)

### MorphNode

```cpp
struct MorphNode {
    uint32_t node_id;                    // node_NNNN
    std::string node_type;               // "div", "button", "text", ...
    MorphStyle style;                    // Computed style
    MorphStyle hover_style;              // :hover override
    MorphStyle active_style;             // :active override
    std::vector<MorphNode*> children;    // Child nodes
    std::vector<IREvent> events;         // onClick, onInput, etc.
    std::string text_content;            // For text nodes
    std::map<std::string, std::string> attrs;  // id, src, href, ...
    
    // Reactivity
    std::map<std::string, std::string> reactive_attrs;
    std::string reactive_text;
    std::string reactive_class;
    std::map<std::string, std::string> reactive_style;
    std::vector<IRConditionalClassEffect> class_conditional_effects;
    
    // Conditional/List
    std::string condition_expr;
    MorphNode* then_node = nullptr;
    MorphNode* else_node = nullptr;
    std::string list_expr;
    std::string list_key_expr;
    MorphNode* item_template = nullptr;
    
    // Animation
    std::vector<IRAnimation> animations;
    std::vector<IRAnimation> hover_animations;
    
    // Layout output
    float x, y, w, h;
    
    // Dirty tracking
    DirtyFlags dirty_flags = DirtyFlags::None;
};
```

### Dirty Flags

```cpp
enum class DirtyFlag : uint32_t {
    None = 0,
    StyleDirty = 1 << 0,      // Style changed, recompute layout
    LayoutDirty = 1 << 1,     // Layout changed, recompute paint
    PaintDirty = 1 << 2,      // Visual changed, repaint
    ScrollDirty = 1 << 3,     // Scroll offset changed
    SubtreeDirty = 1 << 4,    // Child tree changed
};
```

### Flattening (`flatten()`)

Converts the node tree to a flat `RenderFrame` for the compositor thread:

```cpp
struct RenderFrame {
    uint64_t frame_id;
    std::vector<DrawOp> draw_ops;    // Paint commands
    std::vector<NodeLayout> layouts; // Position/size for hit-testing
    std::vector<HoverRule> hover_rules;
    DirtyStats stats;                // Layout/paint/skip/culled counts
};
```

## Rendering Backends

### Flash (Default)

```cpp
// runtime/cpp/renderers/flash/flash.h
void flashCommit(const RenderFrame& frame);   // Build draw ops
void flashPresent();                          // glClear + draw all ops
```

- Full clear each frame
- Replays all `DrawOp` from `RenderFrame`
- ~22 MB @ 1080p, pixel-correct
- Simple, predictable

### Forge (Beta)

```cpp
// runtime/cpp/renderers/forge/forge.h
void forgeCommit(const RenderFrame& frame);   // Build damage + retained layers
void forgePresent();                          // Scissored clears + glBlit
void forgeSetOverlayFn(std::function<void()>); // DevTools overlay
```

- Persistent FBO (retained surface)
- `DamageSet` tracks changed regions:
  - Previous frame geometry map
  - Dirty flags from nodes
  - Compositor animations
  - Scroll content height changes
- Scissored clears per damage rect
- `glBlitFramebuffer` for present
- Idle = blit only (~0 cost)
- ~30 MB floor, targets 5k-20k nodes @ 60Hz

### Selection

```json
// morph.config.json
{ "renderer": "flash" }   // or "forge"

// Dev mode: live toggle in DevTools → Rendering tab
```

## Compositor Thread (`core/compositor.h`)

```
Main Thread                          Compositor Thread
─────────────────                    ───────────────────
Layout + Style                       │
    │                                │
    ▼                                │
flatten() → RenderFrame              │
    │                                │
    ├──── lock-free SPSC queue ────► │
    │                                │
    │                          Wait for vsync
    │                                │
    │                          Interpolate scroll/anim
    │                                │
    │                          Present (glBlit/flashPresent)
    │                                │
    │◄─── feedback (scroll, damage)──┤
```

- Lock-free `spsc_queue.h` (single-producer, single-consumer)
- Main thread produces `RenderFrame`, compositor consumes
- Vsync-aligned presentation
- Scroll interpolation for smooth 60fps scrolling
- Feedback: scroll offset, damage rects back to main thread

## Reactivity (`reactivity/signal.h`)

### Signal<T>

```cpp
template <typename T>
class Signal {
    T value_;
    std::vector<std::function<void()>> subscribers_;
    uint32_t version_ = 0;
    
public:
    T get() { return value_; }
    void set(T new_value) {
        if (value_ != new_value) {
            value_ = std::move(new_value);
            version_++;
            for (auto& fn : subscribers_) fn();
        }
    }
    void subscribe(std::function<void()> fn) { subscribers_.push_back(fn); }
};
```

### create_effect

```cpp
// Tracks which signals are read during execution
thread_local std::vector<SignalBase*>* current_effect_deps = nullptr;

void create_effect(std::function<void()> fn) {
    auto cleanup = [fn]() {
        std::vector<SignalBase*> deps;
        current_effect_deps = &deps;
        fn();
        current_effect_deps = nullptr;
        for (auto* sig : deps) sig->subscribe(cleanup);
    };
    cleanup(); // Initial run
}
```

### create_memo

```cpp
template <typename T>
Signal<T> create_memo(std::function<T()> compute) {
    Signal<T> result(compute());
    create_effect([&] { result.set(compute()); });
    return result;
}
```

## Coroutines (`reactivity/promise.h`, `task.h`)

### morph::Result<T> — Promise<T>

```cpp
template <typename T>
class Result {
    struct promise_type {
        std::optional<T> value;
        std::exception_ptr eptr;
        
        Result get_return_object() { return Result{handle}; }
        std::suspend_never initial_suspend() { return {}; }
        std::suspend_always final_suspend() { return {}; }
        void return_value(T v) { value = std::move(v); }
        void unhandled_exception() { eptr = std::current_exception(); }
    };
    
    // ValueAwaiter: await_ready=true (eager), await_resume extracts value
};
```

### morph::Task — Promise<void>

```cpp
struct Task {
    struct promise_type {
        Task get_return_object() { return Task{handle}; }
        std::suspend_never initial_suspend() { return {}; }
        std::suspend_always final_suspend() { return {}; }
        void return_void() {}
        void unhandled_exception() { std::terminate(); }
    };
    bool done() const { return !handle || handle.done(); }
};
```

### Scheduler

```cpp
// task.h
void process_tasks();           // Drive all coroutines + timers
void schedule_coroutine(Task);  // Spawn detached task
struct next_frame {             // await next_frame
    bool await_ready() { return false; }
    bool await_suspend(std::coroutine_handle<> h);
    void await_resume() {}
};

// Timer API
int set_timeout(std::function<void()> fn, int ms);
int set_interval(std::function<void()> fn, int ms);
void clear_timer(int id);
```

## Networking (`net/net.h`)

### HttpAwaitable

```cpp
struct HttpAwaitable {
    std::shared_ptr<SharedState> state;
    bool await_ready() { return false; }
    void await_suspend(std::coroutine_handle<> h) {
        // Spawn worker thread: blocking HTTP via libcurl or socket
        // On completion: h.resume()
    }
    Response await_resume() {
        if (error) throw JsValue(ErrorObject);
        return state->response;
    }
};
```

### fetch()

```cpp
inline morph::Result<JsString> fetch(const std::string& url) {
    auto state = std::make_shared<SharedState>();
    state->url = url;
    return [state]() -> morph::Result<JsString> {
        co_return (co_await HttpAwaitable{state}).text();
    }();
}
```

- Worker thread pool for blocking HTTP
- Response resumes coroutine on main thread
- `Response` mirrors browser API: `status`, `ok()`, `text()`, `headers`, `json()`, `clone()`

## JS Types (`types/`)

### JsValue (Variant)

```cpp
struct JsValue {
    std::variant<JsUndefined, JsNull, JsBoolean, JsNumber, JsString, JsArray, JsObject, JsFunction> inner;
    
    // Type checks
    bool is_undefined() const;
    bool is_number() const;
    // ...
    
    // JS semantics
    bool truthy() const;          // JS truthiness
    std::string typeof_() const;  // JS typeof
    bool operator==(const JsValue&) const;  // JS ==
    JsString toString() const;    // JS toString()
    
    // Property access
    JsValue get(const std::string& key) const;
    JsValue operator[](const std::string& key) const;
    JsValue& operator[](const std::string& key);  // mutable
    
    // Coercion
    operator std::string() const; // JS string conversion
};
```

### JsNumber (int64/double/bigint)

```cpp
struct JsNumber {
    std::variant<int64_t, double, std::string> value;  // bigint as string
    
    bool is_int() const { return std::holds_alternative<int64_t>(value); }
    bool is_big() const { return std::holds_alternative<std::string>(value); }
    
    int64_t as_int() const;
    double as_double() const;
    std::string as_string() const;
    
    // Arithmetic with int64 fast-path
    JsNumber operator+(const JsNumber&) const;
    JsNumber operator-(const JsNumber&) const;
    // ...
};
```

### JsArray / JsObject (Shared Storage)

```cpp
struct JsArray {
    std::shared_ptr<std::vector<JsValue>> elements;
    size_t length() const { return elements->size(); }
    void push(const JsValue& v) { elements->push_back(v); }
    JsValue pop();
    JsValue operator[](int64_t idx) const;  // OOB → undefined
    JsValue& operator[](int64_t idx);       // OOB → thread-local dummy
    auto begin() const { return elements->begin(); }
    auto end() const { return elements->end(); }
};

struct JsObject {
    std::shared_ptr<std::map<std::string, JsValue>> properties;
    JsValue get(const std::string& key) const;
    void set(const std::string& key, const JsValue& val);
    bool has(const std::string& key) const;
    std::vector<std::string> keys() const;
    JsValue operator[](const std::string& key) const;
    JsValue& operator[](const std::string& key);
};
```

## Style System (`style/`)

### Feature Gating

```cpp
// style.h
struct MorphStyle : StyleBase
    #ifdef MORPH_FEATURE_FLEX
    , FlexStyle
    #endif
    #ifdef MORPH_FEATURE_POSITION
    , PositionStyle
    #endif
    // ... etc
{};
```

### CSS Cascade (in IRBuilder)

```cpp
// morph-ir/src/builder.rs
fn apply_css_prop(node, prop, value) {
    // 1. UA defaults (h1: 32px bold, button: inline-block, etc.)
    // 2. CSS file rules (source order, selector specificity)
    // 3. Tailwind utilities (resolved to props)
    // 4. Inline style={{}} (highest priority)
    // 5. Reactive style (runtime override)
}
```

### Units

```cpp
// style/features/base.h
float parse_length(const std::string& s) {
    if (ends_with(s, "px")) return parse(s);
    if (ends_with(s, "rem")) return parse(s) * 16;
    if (ends_with(s, "em")) return parse(s) * parent_font_size;
    if (ends_with(s, "%")) return None;  // Handled in layout
    if (ends_with(s, "vh")) return screen_height * parse(s) / 100;
    if (ends_with(s, "vw")) return screen_width * parse(s) / 100;
    return parse(s);  // Bare number = px
}
```

## DevTools (`dev/inspector.h`)

```cpp
struct DevTools {
    bool open = false;
    bool inspecting = false;
    MorphNode* hovered_node = nullptr;
    MorphNode* selected_node = nullptr;
    Tab active_tab = Tab::Elements;  // Elements, Rendering, Network, Logs
    
    void draw();           // Main draw call
    void toggle();         // F12
    void toggle_inspect(); // F2
    void draw_panel();     // Docked panel
    void draw_overlay();   // Box model (margin/border/padding/content rings)
};
```

- Compiled only in `morph_devrt` (via `MORPH_DEV_RUNTIME` define)
- Zero cost in production builds
- State preserved across hot reloads

## Adding a New Widget

1. **Create C++ node** in `ui/`:
```cpp
// ui/my_widget.h
struct MyWidgetNode : MorphNode {
    MyWidgetNode() { node_type = "my-widget"; }
    void layout() override { /* custom layout */ }
    void paint(DrawOpList& ops) override { /* custom draw ops */ }
};
```

2. **Register in IRBuilder** (`morph-ir/src/builder.rs`):
```rust
"my-widget" => IRNodeType::Custom("MyWidgetNode"),
```

3. **Add to FeatureSet** (`morph-codegen/src/cpp/feature_set.rs`):
```rust
Feature::Custom("MyWidgetNode") => {
    required_headers.push("ui/my_widget.h");
}
```

4. **Export to JSX** (optional):
```cpp
// In logic emitter: allow `import { MyWidget } from 'morph'`
```

## Extending CSS Properties

1. Add to `css_registry.rs`: `KNOWN_PROPERTIES`, `CSS_TO_IR`
2. Add parser in `style/features/` or `style/style.h`
3. Add to `IRStyle` struct
4. Handle in layout/paint

## Performance Tips

| Area | Tip |
|---|---|
| **Layout** | Avoid `%` sizes on deep trees; use flex `gap` not margins |
| **Paint** | `display: none` skips subtree; `visibility: hidden` still layouts |
| **Reactivity** | Narrow effect deps; use `create_memo` for derived state |
| **Coroutines** | `co_await fetch()` yields; don't block in async fns |
| **Images** | Reuse `src` URLs; textures cached by URL |
| **Text** | Static text baked at layout; dynamic text re-measures |

## Build Configuration

```bash
# Feature defines (from FeatureSet scan)
-DMORPH_FEATURE_TEXT
-DMORPH_FEATURE_FLEX
-DMORPH_FEATURE_SCROLL
-DMORPH_FEATURE_BUTTON
-DMORPH_RENDERER_FLASH    # or FORGE

# Compiler flags
-std=c++20 -O2 -ffunction-sections -fdata-sections
-Wl,--gc-sections

# Linker
-lglfw -lfreetype -lharfbuzz -lGL -lpthread -ldl
# Windows: -lopengl32 -lgdi32 -luser32 -lshell32
# macOS: -framework Cocoa -framework OpenGL -framework IOKit -framework CoreVideo
```