#include "../node.h"
#include "../renderer.h"
#include <cstring>
#include <chrono>

void MorphNode::recordDisplayList(Renderer& r) {
}

void MorphNode::executeDisplayList(Renderer& r) {
    draw(r);
}

static uint8_t overflowToEnum(const std::string& s) {
    if (s == "hidden") return 1;
    if (s == "scroll") return 2;
    if (s == "auto")   return 3;
    return 0;
}
static uint8_t boxSizingToEnum(const std::string& s) {
    return (s == "border-box") ? 1 : 0;
}
static uint8_t displayToEnum(const std::string& s) {
    if (s == "flex")  return 1;
    if (s == "none")  return 2;
    if (s == "inline") return 3;
    return 0;
}
static uint8_t positionToEnum(const std::string& s) {
    return (s == "absolute") ? 1 : 0;
}
static uint8_t fontWeightToEnum(const std::string& s) {
    return (s == "bold" || s == "700" || s == "800" || s == "900") ? 1 : 0;
}
static uint8_t borderStyleToEnum(const std::string& s) {
    return (s == "solid") ? 1 : 0;
}

int MorphNode::flattenExtra(RenderFrame& frame, FlatRenderNode& fn) {
    (void)frame; (void)fn;
    return 0;
}

bool MorphNode::subtreeMayMove() const {
    if (m_isTransitioning || m_hasLayoutTransition)
        return true;
    for (const auto& a : m_animations)
        if (a.running && !a.finished)
            return true;
    for (auto* c : children)
        if (c->subtreeMayMove())
            return true;
    return false;
}

int MorphNode::flatten(RenderFrame& frame, int parentId, float scrollOffset) {
    // Off-screen culling: skip anything whose box is fully outside the scene
    // viewport. Node coords are absolute root-space but do NOT include scroll:
    // a scrolling ancestor shifts the whole subtree at draw time via
    // pushScrollOffset(0, -scrollY). So the effective screen Y is
    // y - scrollOffset, where scrollOffset accumulates the scrollY of every
    // scrolling ancestor on the path (set per-child below).
    float sy = y - scrollOffset;
    bool offscreen = frame.viewW > 0.0f && frame.viewH > 0.0f &&
                     (x + w <= 0.0f || x >= frame.viewW ||
                      sy + h <= 0.0f || sy >= frame.viewH);
    if (offscreen)
    {
        bool clips = style.overflow == "hidden" || style.overflow == "scroll" ||
                     style.overflow == "auto" || style.borderRadius > 0.0f;
        // Clipping nodes fully contain their descendants. If nothing in the
        // subtree can move (running animation/transition), it can never
        // become visible — drop the whole subtree. Non-clipping nodes still
        // emit an empty shell below so overflowed descendants keep their
        // parent structure (and their own cull test).
        if (clips && !subtreeMayMove())
        {
            frame.culledCount++;
            return -1;
        }
    }

    int idx = (int)frame.nodes.size();
    FlatRenderNode fn;
    fn.id = idx;
    fn.parentId = parentId;
    fn.x = x; fn.y = y; fn.w = w; fn.h = h;
    fn.isTransitioning = m_isTransitioning;
    fn.hasLayoutTransition = m_hasLayoutTransition;

    memcpy(fn.bgColor, style.bgColor, sizeof(float)*4);
    memcpy(fn.color, style.color, sizeof(float)*4);
    fn.borderRadius = style.borderRadius;
    fn.borderWidth = 0.0f;
    fn.borderColor[0] = fn.borderColor[1] = fn.borderColor[2] = 0.0f; fn.borderColor[3] = 1.0f;
    fn.borderStyle = 0;
#ifdef MORPH_FEATURE_BORDER
    fn.borderWidth = style.borderWidth;
    memcpy(fn.borderColor, style.borderColor, sizeof(float)*4);
    fn.borderStyle = borderStyleToEnum(style.borderStyle);
#endif

    fn.overflow = overflowToEnum(style.overflow);
    fn.boxSizing = boxSizingToEnum(style.boxSizing);
    fn.display = displayToEnum(style.display);
    fn.position = positionToEnum(style.position);

    fn.fontSize = style.fontSize;
    fn.textAlign = (style.textAlign == "center") ? (uint8_t)1 : (style.textAlign == "right" ? (uint8_t)2 : (uint8_t)0);
    fn.fontWeight = fontWeightToEnum(style.fontWeight);

    fn.scrollY = scrollY;
    fn.contentH = contentH;
    fn.scrollEnabled = scrollEnabled;
    fn.scrollbarWidth = 8.0f;
    { float c[4] = {0.85f,0.85f,0.85f,0.4f}; memcpy(fn.scrollbarTrackColor, c, sizeof(float)*4); }
    { float c[4] = {0.5f,0.5f,0.5f,0.6f}; memcpy(fn.scrollbarThumbColor, c, sizeof(float)*4); }
    fn.scrollbarBorderRadius = 4.0f;
#ifdef MORPH_FEATURE_SCROLL
    fn.scrollbarWidth = style.scrollbarWidth;
    memcpy(fn.scrollbarTrackColor, style.scrollbarTrackColor, sizeof(float)*4);
    memcpy(fn.scrollbarThumbColor, style.scrollbarThumbColor, sizeof(float)*4);
    fn.scrollbarBorderRadius = style.scrollbarBorderRadius;
#endif

    int dlStart = (int)frame.drawOps.size();
    if (offscreen)
    {
        // Shell node: keep the tree structure but skip the draw payload.
        frame.culledCount++;
    }
    else
    {
        frame.drawOps.insert(frame.drawOps.end(), m_displayList.begin(), m_displayList.end());
    }
    fn.dlOffset = dlStart;
    fn.dlCount = (int)frame.drawOps.size() - dlStart;

    fn.textOpOffset = (int)frame.textOps.size();
    if (!offscreen)
        fn.textOpCount = flattenExtra(frame, fn);

    auto now = std::chrono::steady_clock::now().time_since_epoch().count();
    double nowSec = (double)now / 1000000000.0;
    for (auto& a : m_animations) {
        if (!a.running || a.finished) continue;
        CompositorAnimProperty cap;
        switch (a.property) {
            case AnimProperty::X: cap = CompositorAnimProperty::X; break;
            case AnimProperty::Y: cap = CompositorAnimProperty::Y; break;
            case AnimProperty::BgColorR: cap = CompositorAnimProperty::BgColorR; break;
            case AnimProperty::BgColorG: cap = CompositorAnimProperty::BgColorG; break;
            case AnimProperty::BgColorB: cap = CompositorAnimProperty::BgColorB; break;
            case AnimProperty::BgColorA: cap = CompositorAnimProperty::BgColorA; break;
            case AnimProperty::ColorR: cap = CompositorAnimProperty::ColorR; break;
            case AnimProperty::ColorG: cap = CompositorAnimProperty::ColorG; break;
            case AnimProperty::ColorB: cap = CompositorAnimProperty::ColorB; break;
            case AnimProperty::ColorA: cap = CompositorAnimProperty::ColorA; break;
            case AnimProperty::BorderRadius: cap = CompositorAnimProperty::BorderRadius; break;
            default: continue;
        }
        double startTime = nowSec - (double)a.elapsed;
        frame.animations.push_back({idx, cap, a.from, a.to,
                                    startTime, a.duration, (uint8_t)a.easing, true});
    }

    frame.nodes.push_back(fn);

    // Accumulate this node's scroll for descendants: matches the renderer's
    // pushScrollOffset, which is applied only when the content overflows
    // (scrollEnabled && contentH > h).
    float childScroll = scrollOffset +
                        (scrollEnabled && contentH > h ? scrollY : 0.0f);

    for (auto* child : paintOrder()) {
        int childIdx = child->flatten(frame, idx, childScroll);
        if (childIdx >= 0)
            frame.nodes[idx].children.push_back(childIdx);
    }

    return idx;
}
