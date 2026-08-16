#pragma once
#include <vector>
#include <cstdint>
#include <string>
#include <atomic>
#include <mutex>
#include "draw_op.h"
#include "event.h"

enum class Easing : uint8_t {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
};

// ── Animation property (subset that can run on compositor) ──
enum class CompositorAnimProperty : uint8_t {
    X, Y,
    BgColorR, BgColorG, BgColorB, BgColorA,
    ColorR, ColorG, ColorB, ColorA,
    BorderRadius,
    Opacity,
};

// ── Animation state (written by main thread, read by compositor) ──
struct AnimationState {
    int nodeId;
    CompositorAnimProperty prop;
    float from, to;
    double startTime;   // absolute time (steady_clock seconds)
    float duration;
    uint8_t easing;     // Easing enum as uint8
    bool running;
    bool reported = false;  // compositor sets true after pushing completion event
};

// ── Flat render node (pointer-free snapshot for compositor) ──
struct FlatRenderNode {
    int id;
    int parentId;

    // Layout-computed base position
    float x, y, w, h;

    // Animatable style fields (snapshot at paint time)
    float bgColor[4];
    float color[4];
    float borderRadius;
    float borderWidth;
    float borderColor[4];
    uint8_t borderStyle;    // 0=none, 1=solid

    uint8_t overflow;       // 0=visible, 1=hidden, 2=scroll, 3=auto
    uint8_t boxSizing;      // 0=content-box, 1=border-box
    uint8_t display;        // 0=block, 1=flex, 2=none, 3=inline
    uint8_t position;       // 0=static, 1=absolute

    float fontSize;
    uint8_t textAlign;      // 0=left, 1=center, 2=right
    uint8_t fontWeight;     // 0=normal, 1=bold

    // Scroll
    float scrollY;
    float contentH;
    bool scrollEnabled;
    float scrollbarWidth;
    float scrollbarTrackColor[4];
    float scrollbarThumbColor[4];
    float scrollbarBorderRadius;

    // True while this node has an active transition (set during flatten, no propagation)
    bool isTransitioning = false;
    // True when active transition changes layout-affecting properties
    bool hasLayoutTransition = false;

    // Compositor-written fields (updated each vsync tick):
    float animOffsetX = 0;
    float animOffsetY = 0;
    float animOpacity = 1.0f;

#ifdef MORPH_FEATURE_TRANSFORM
    // Resolved transform (identity when transformSet == false)
    bool transformSet = false;
    float matrix[16] = {1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1};
    // transform-origin as fractions of this node's box (CSS default center).
    float originX = 0.5f, originY = 0.5f;
    // Screen-space AABB of the node's box under its full accumulated
    // transform (own + ancestor matrices + scroll offsets).
    float cullX = 0, cullY = 0, cullW = 0, cullH = 0;
#endif

    // Display list (pre-recorded paint commands)
    int dlOffset;
    int dlCount;

    // Text rendering (separate from drawOps because DrawOp is POD)
    int textOpOffset = 0;
    int textOpCount = 0;

    // Children (indices into frame's nodes array)
    std::vector<int> children;
};

struct FlatTextOp {
    int nodeId;
    std::string text;
    float x, y;
    float color[4];
    uint8_t align;      // 0=left, 1=center, 2=right
    float fontSize;
    uint8_t fontWeight; // 0=normal, 1=bold
};

// ── Event struct for animation completion feedback ──
struct AnimCompletionEvent {
    int nodeId;
    CompositorAnimProperty prop;
};

// ── Texture upload job (main thread → compositor) ──
struct UploadJob {
    std::string path;
    int width, height;
    unsigned char* pixels;   // compositor must free() after upload
};

// ── One committed frame ──
struct RenderFrame {
    std::vector<FlatRenderNode> nodes;
    std::vector<DrawOp> drawOps;
    std::vector<AnimationState> animations;
    std::vector<FlatTextOp> textOps;
    uint64_t frameId;
    double timestamp;

    // Scene viewport in screen space (set by the commit path before flatten;
    // used for off-screen culling at flatten time).
    float viewW = 0.0f;
    float viewH = 0.0f;
    // Nodes whose draw/text payload was skipped this frame (off-screen cull).
    int culledCount = 0;
};

// ── Global double-buffered frame state (main thread writes, compositor reads) ──
inline std::atomic<RenderFrame*> g_frontFrame{nullptr};
inline RenderFrame g_backFrames[2];
inline std::atomic<int> g_backIndex{0};
inline std::atomic<bool> g_framePending{false};
inline std::atomic<bool> g_frameInterpolated{false};

// ── SPSC event queues (lock-free) ──
#include "spsc_queue.h"

// GLFW events → main thread (declared here for access by window callbacks)
inline SPSCQueue<MorphEvent, 64> g_eventQueue;

// Compositor → main thread (animation completion feedback)
inline SPSCQueue<AnimCompletionEvent, 64> g_feedbackQueue;
