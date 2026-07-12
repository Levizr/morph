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
    return (s == "bold") ? 1 : 0;
}
static uint8_t borderStyleToEnum(const std::string& s) {
    return (s == "solid") ? 1 : 0;
}

int MorphNode::flattenExtra(RenderFrame& frame, FlatRenderNode& fn) {
    (void)frame; (void)fn;
    return 0;
}

int MorphNode::flatten(RenderFrame& frame, int parentId) {
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
    frame.drawOps.insert(frame.drawOps.end(), m_displayList.begin(), m_displayList.end());
    fn.dlOffset = dlStart;
    fn.dlCount = (int)m_displayList.size();

    fn.textOpOffset = (int)frame.textOps.size();
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

    for (auto* child : children) {
        int childIdx = child->flatten(frame, idx);
        frame.nodes[idx].children.push_back(childIdx);
    }

    return idx;
}
