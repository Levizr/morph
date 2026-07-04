#include "node.h"
#include "renderer.h"
#include <cmath>
#include <cstring>
#include <chrono>

// ── Dirty flag propagation ───────────────────────────────────
void MorphNode::markDirty(DirtyFlag f) {
    if (f == Clean) return;
    m_dirtyFlags |= f;
    // Propagate layout/subtree dirtiness up the tree
    if ((f == LayoutDirty || f == SubtreeDirty) && parent && !parent->isDirty(SubtreeDirty)) {
        parent->markDirty(SubtreeDirty);
    }
    if (f == PaintDirty && parent && !parent->isDirty(PaintDirty)) {
        parent->markDirty(PaintDirty);
    }
}

void MorphNode::layoutIfNeeded(float px, float py, float parentW, float parentH,
                                Renderer* r, DirtyStats* stats, bool force) {
    bool selfDirty = isDirty(LayoutDirty) || isDirty(StyleDirty);
    bool subtreeDirty = isDirty(SubtreeDirty);
    bool needsLayout = selfDirty || subtreeDirty || force;

    if (!needsLayout && stats) stats->skippedCount++;

    if (needsLayout) {
        if (stats) {
            stats->layoutCount++;
            if (selfDirty && !subtreeDirty)
                stats->paintCount++; // pure layout
        }
        layout(px, py, parentW, parentH, r);
        clearDirty(LayoutDirty);
        clearDirty(StyleDirty);
        // After layout, mark self as needing a display list re-record
        markDirty(PaintDirty);
    }

    for (auto* c : children) {
        float cw = c->w > 0 ? c->w : (parentW - c->x + px);
        float ch = c->h > 0 ? c->h : (parentH - c->y + py);
        // Propagate force down so entire dirty subtree re-layouts
        c->layoutIfNeeded(c->x, c->y, cw, ch, r, stats, force || subtreeDirty);
    }
    if (needsLayout) clearDirty(SubtreeDirty);
}

// ── Animation helpers ─────────────────────────────────────────
static float applyEasing(float t, Easing e) {
    switch (e) {
        case Easing::Linear:   return t;
        case Easing::EaseIn:   return t * t;
        case Easing::EaseOut:  return 1.0f - (1.0f - t) * (1.0f - t);
        case Easing::EaseInOut: return t < 0.5f ? 2.0f * t * t : 1.0f - (float)pow(-2.0f * t + 2.0f, 2.0f) / 2.0f;
    }
    return t;
}

static void setAnimProperty(MorphNode* node, AnimProperty prop, float val) {
    switch (prop) {
        case AnimProperty::X: node->x = val; node->markDirty(LayoutDirty); break;
        case AnimProperty::Y: node->y = val; node->markDirty(LayoutDirty); break;
        case AnimProperty::W: node->w = val; node->markDirty(LayoutDirty); break;
        case AnimProperty::H: node->h = val; node->markDirty(LayoutDirty); break;
        case AnimProperty::BgColorR: node->style.bgColor[0] = val; node->markDirty(PaintDirty); break;
        case AnimProperty::BgColorG: node->style.bgColor[1] = val; node->markDirty(PaintDirty); break;
        case AnimProperty::BgColorB: node->style.bgColor[2] = val; node->markDirty(PaintDirty); break;
        case AnimProperty::BgColorA: node->style.bgColor[3] = val; node->markDirty(PaintDirty); break;
        case AnimProperty::ColorR: node->style.color[0] = val; node->markDirty(PaintDirty); break;
        case AnimProperty::ColorG: node->style.color[1] = val; node->markDirty(PaintDirty); break;
        case AnimProperty::ColorB: node->style.color[2] = val; node->markDirty(PaintDirty); break;
        case AnimProperty::ColorA: node->style.color[3] = val; node->markDirty(PaintDirty); break;
        case AnimProperty::BorderRadius: node->style.borderRadius = val; node->markDirty(PaintDirty); break;
    }
}

static float getAnimProperty(MorphNode* node, AnimProperty prop) {
    switch (prop) {
        case AnimProperty::X: return node->x;
        case AnimProperty::Y: return node->y;
        case AnimProperty::W: return node->w;
        case AnimProperty::H: return node->h;
        case AnimProperty::BgColorR: return node->style.bgColor[0];
        case AnimProperty::BgColorG: return node->style.bgColor[1];
        case AnimProperty::BgColorB: return node->style.bgColor[2];
        case AnimProperty::BgColorA: return node->style.bgColor[3];
        case AnimProperty::ColorR: return node->style.color[0];
        case AnimProperty::ColorG: return node->style.color[1];
        case AnimProperty::ColorB: return node->style.color[2];
        case AnimProperty::ColorA: return node->style.color[3];
        case AnimProperty::BorderRadius: return node->style.borderRadius;
    }
    return 0;
}

void MorphNode::startAnimation(AnimProperty prop, float to, float duration, Easing easing) {
    for (auto& a : m_animations) {
        if (a.property == prop) {
            a.from = getAnimProperty(this, prop);
            a.to = to;
            a.duration = duration;
            a.elapsed = 0;
            a.easing = easing;
            a.running = true;
            a.finished = false;
            markDirty(PaintDirty);
            return;
        }
    }
    MorphAnimation a;
    a.property = prop;
    a.from = getAnimProperty(this, prop);
    a.to = to;
    a.duration = duration;
    a.elapsed = 0;
    a.easing = easing;
    a.running = true;
    a.finished = false;
    m_animations.push_back(a);
    markDirty(PaintDirty);
}

void MorphNode::updateAnimations(float dt) {
    for (auto& a : m_animations) {
        if (!a.running || a.finished) continue;
        a.elapsed += dt;
        float t = a.elapsed / a.duration;
        if (t >= 1.0f) {
            t = 1.0f;
            a.finished = true;
            a.running = false;
            // Snap to final value on main thread (for correctness)
            float val = a.to;
            setAnimProperty(this, a.property, val);
        }
    }
    // Running animations are interpolated by the compositor thread now.
    // We only snap on completion here so the node's properties settle
    // at the correct final value after the compositor's work is done.
}

// Helper: apply only non-default fields from delta to target
static void applyStyleDelta(MorphStyle& target, const MorphStyle& delta) {
    if (delta.bgColor[0] != 0.0f || delta.bgColor[1] != 0.0f || delta.bgColor[2] != 0.0f || delta.bgColor[3] != 0.0f)
        memcpy(target.bgColor, delta.bgColor, sizeof(float)*4);
    if (delta.color[0] != 0.0f || delta.color[1] != 0.0f || delta.color[2] != 0.0f || delta.color[3] != 1.0f)
        memcpy(target.color, delta.color, sizeof(float)*4);
    if (delta.borderRadius != 0.0f) target.borderRadius = delta.borderRadius;
    if (delta.fontSize != 16.0f)     target.fontSize = delta.fontSize;
    if (delta.fontWeight != "normal") target.fontWeight = delta.fontWeight;
    if (delta.textAlign != "left")    target.textAlign = delta.textAlign;
    if (delta.display != "block")     target.display = delta.display;
    if (delta.overflow != "visible")  target.overflow = delta.overflow;
    if (delta.position != "static")   target.position = delta.position;
    if (delta.boxSizing != "content-box") target.boxSizing = delta.boxSizing;
    // Spacing
    if (delta.padding[0] != 0.0f || delta.padding[1] != 0.0f || delta.padding[2] != 0.0f || delta.padding[3] != 0.0f)
        memcpy(target.padding, delta.padding, sizeof(float)*4);
    if (delta.margin[0] != 0.0f || delta.margin[1] != 0.0f || delta.margin[2] != 0.0f || delta.margin[3] != 0.0f)
        memcpy(target.margin, delta.margin, sizeof(float)*4);
    if (delta.marginAuto[0] || delta.marginAuto[1] || delta.marginAuto[2] || delta.marginAuto[3])
        memcpy(target.marginAuto, delta.marginAuto, sizeof(bool)*4);
    // Sizing
    if (delta.explicitWidth >= 0.0f)  target.explicitWidth = delta.explicitWidth;
    if (delta.explicitHeight >= 0.0f) target.explicitHeight = delta.explicitHeight;
    if (delta.minWidth >= 0.0f)       target.minWidth = delta.minWidth;
    if (delta.maxWidth >= 0.0f)       target.maxWidth = delta.maxWidth;
    if (delta.minHeight >= 0.0f)      target.minHeight = delta.minHeight;
    if (delta.maxHeight >= 0.0f)      target.maxHeight = delta.maxHeight;
    // Flex
#ifdef MORPH_FEATURE_FLEX
    if (delta.flexDirection != "row")  target.flexDirection = delta.flexDirection;
    if (delta.gap != 0.0f)             target.gap = delta.gap;
    if (delta.justifyContent != "flex-start") target.justifyContent = delta.justifyContent;
    if (delta.alignItems != "stretch") target.alignItems = delta.alignItems;
    if (delta.flexWrap != "nowrap")    target.flexWrap = delta.flexWrap;
    if (delta.flexGrow != 0.0f)        target.flexGrow = delta.flexGrow;
    if (delta.flexShrink != 1.0f)      target.flexShrink = delta.flexShrink;
    if (delta.flexBasis != "auto")     target.flexBasis = delta.flexBasis;
#endif
    // Position
#ifdef MORPH_FEATURE_POSITION
    if (delta.left > -1e8f)  target.left = delta.left;
    if (delta.right > -1e8f) target.right = delta.right;
    if (delta.top > -1e8f)   target.top = delta.top;
    if (delta.bottom > -1e8f) target.bottom = delta.bottom;
#endif
    // Cursor
#ifdef MORPH_FEATURE_CURSOR
    if (delta.cursor != "default") target.cursor = delta.cursor;
#endif
    // Border
#ifdef MORPH_FEATURE_BORDER
    if (delta.borderWidth > 0.0f) target.borderWidth = delta.borderWidth;
    if (delta.borderColor[0] != 0.0f || delta.borderColor[1] != 0.0f || delta.borderColor[2] != 0.0f || delta.borderColor[3] != 1.0f)
        memcpy(target.borderColor, delta.borderColor, sizeof(float)*4);
    if (delta.borderStyle != "none") target.borderStyle = delta.borderStyle;
#endif
    // Scroll
#ifdef MORPH_FEATURE_SCROLL
    if (delta.scrollbarWidth != 8.0f) target.scrollbarWidth = delta.scrollbarWidth;
    if (delta.scrollbarTrackColor[0] != 0.85f || delta.scrollbarTrackColor[1] != 0.85f || delta.scrollbarTrackColor[2] != 0.85f || delta.scrollbarTrackColor[3] != 0.4f)
        memcpy(target.scrollbarTrackColor, delta.scrollbarTrackColor, sizeof(float)*4);
    if (delta.scrollbarThumbColor[0] != 0.5f || delta.scrollbarThumbColor[1] != 0.5f || delta.scrollbarThumbColor[2] != 0.5f || delta.scrollbarThumbColor[3] != 0.6f)
        memcpy(target.scrollbarThumbColor, delta.scrollbarThumbColor, sizeof(float)*4);
    if (delta.scrollbarBorderRadius != 4.0f) target.scrollbarBorderRadius = delta.scrollbarBorderRadius;
#endif
}

void MorphNode::onHover(bool state) {
    if (hoverStyle) {
        if (!m_hoverTransition)
            m_hoverTransition = new HoverTransition();
        if (state) {
            // Only snapshot on fresh hover entry, not during re-entry
            // from child-propagation while a revert animation is running
            if (!m_hoverTransition->active)
                m_hoverTransition->preHoverStyle = style;
            if (m_transitionDuration > 0.0f) {
                m_hoverTransition->startStyle = style;
                m_hoverTransition->targetStyle = style;
                applyStyleDelta(m_hoverTransition->targetStyle, *hoverStyle);
                m_hoverTransition->elapsed = 0.0f;
                m_hoverTransition->active = true;
            } else {
                applyStyleDelta(style, *hoverStyle);
            }
        } else {
            if (m_transitionDuration > 0.0f) {
                m_hoverTransition->startStyle = style;
                m_hoverTransition->targetStyle = m_hoverTransition->preHoverStyle;
                m_hoverTransition->elapsed = 0.0f;
                m_hoverTransition->active = true;
            } else {
                style = m_hoverTransition->preHoverStyle;
            }
        }
        markDirty(PaintDirty);
        markDirty(LayoutDirty);
    }
    // Check ancestor-hover rules on descendants when this node's hover toggles
    _applyAncestorHover(state);
    // Propagate to ancestors: CSS says child hover applies :hover to all ancestors
    if (parent) parent->onHover(state);
}

static void _applyOneAncestorRule(MorphNode* child, const AncestorHoverRule& rule, bool state) {
    auto t = child->m_ancestorHoverTransition;
    if (state) {
        if (!t) {
            t = new AncestorHoverTransition();
            child->m_ancestorHoverTransition = t;
        }
        if (t->applyCount == 0)
            t->revertStyle = child->style;
        t->applyCount++;

        if (child->m_transitionDuration > 0.0f && t->active) {
            t->targetStyle = child->style;
            applyStyleDelta(t->targetStyle, rule.style);
            t->applying = true;
            t->elapsed = 0.0f;
        } else if (child->m_transitionDuration > 0.0f) {
            t->targetStyle = child->style;
            applyStyleDelta(t->targetStyle, rule.style);
            t->applying = true;
            t->elapsed = 0.0f;
            t->active = true;
        } else {
            applyStyleDelta(child->style, rule.style);
        }
        child->markDirty(PaintDirty);
        child->markDirty(LayoutDirty);
    } else {
        if (!t) return; // nothing to revert
        t->applyCount--;
        if (t->applyCount > 0) return; // other ancestor rules still active

        if (t->active) {
            t->applying = false;
            t->elapsed = 0.0f;
        } else if (child->m_transitionDuration > 0.0f) {
            t->applying = false;
            t->elapsed = 0.0f;
            t->active = true;
        } else {
            child->style = t->revertStyle;
        }
        child->markDirty(PaintDirty);
        child->markDirty(LayoutDirty);
    }
}

void MorphNode::_applyAncestorHover(bool state) {
    for (auto* child : children) {
        child->_applyAncestorHover(state);
        for (auto& rule : child->m_ancestorHoverRules) {
            // Match: ancestorTag is empty (any ancestor) or matches this node's type
            if (rule.ancestorTag.empty() || type == rule.ancestorTag) {
                _applyOneAncestorRule(child, rule, state);
            }
        }
    }
}

void MorphNode::updateHoverTransition(float dt) {
    if (m_hoverTransition && m_hoverTransition->active) {
        m_hoverTransition->elapsed += dt;
        float t = m_hoverTransition->elapsed / m_transitionDuration;
        if (t >= 1.0f) {
            t = 1.0f;
            style = m_hoverTransition->targetStyle;
            m_hoverTransition->active = false;
        } else {
            interpolateStyles(style, m_hoverTransition->startStyle,
                              m_hoverTransition->targetStyle,
                              applyEasing(t, m_transitionEasing));
        }
        markDirty(PaintDirty);
        markDirty(LayoutDirty);
    }
}

void MorphNode::interpolateStyles(MorphStyle& out, const MorphStyle& a,
                                   const MorphStyle& b, float t) {
    for (int i = 0; i < 4; i++) {
        out.bgColor[i] = a.bgColor[i] + (b.bgColor[i] - a.bgColor[i]) * t;
        out.color[i] = a.color[i] + (b.color[i] - a.color[i]) * t;
        out.padding[i] = a.padding[i] + (b.padding[i] - a.padding[i]) * t;
        out.margin[i] = a.margin[i] + (b.margin[i] - a.margin[i]) * t;
        out.marginAuto[i] = b.marginAuto[i]; // snap
    }
    out.borderRadius = a.borderRadius + (b.borderRadius - a.borderRadius) * t;
    out.fontSize = a.fontSize + (b.fontSize - a.fontSize) * t;

    // Width/height: interpolate only if both are explicitly set
    auto lerpIfSet = [t](float av, float bv) {
        return (av >= 0.0f && bv >= 0.0f) ? av + (bv - av) * t : bv;
    };
    out.explicitWidth = lerpIfSet(a.explicitWidth, b.explicitWidth);
    out.explicitHeight = lerpIfSet(a.explicitHeight, b.explicitHeight);
    out.minWidth = lerpIfSet(a.minWidth, b.minWidth);
    out.maxWidth = lerpIfSet(a.maxWidth, b.maxWidth);
    out.minHeight = lerpIfSet(a.minHeight, b.minHeight);
    out.maxHeight = lerpIfSet(a.maxHeight, b.maxHeight);

    // Strings: snap to target
    out.fontWeight = b.fontWeight;
    out.overflow = b.overflow;
    out.display = b.display;
    out.position = b.position;
    out.textAlign = b.textAlign;
    out.boxSizing = b.boxSizing;

#ifdef MORPH_FEATURE_BORDER
    out.borderWidth = a.borderWidth + (b.borderWidth - a.borderWidth) * t;
    for (int i = 0; i < 4; i++)
        out.borderColor[i] = a.borderColor[i] + (b.borderColor[i] - a.borderColor[i]) * t;
    out.borderStyle = b.borderStyle; // snap
#endif

#ifdef MORPH_FEATURE_FLEX
    out.gap = a.gap + (b.gap - a.gap) * t;
    out.flexDirection = b.flexDirection;
    out.justifyContent = b.justifyContent;
    out.alignItems = b.alignItems;
    out.flexWrap = b.flexWrap;
#endif

#ifdef MORPH_FEATURE_POSITION
    auto lerpIfSetPos = [t](float av, float bv) {
        return (av > -1e8f && bv > -1e8f) ? av + (bv - av) * t : bv;
    };
    out.left = lerpIfSetPos(a.left, b.left);
    out.right = lerpIfSetPos(a.right, b.right);
    out.top = lerpIfSetPos(a.top, b.top);
    out.bottom = lerpIfSetPos(a.bottom, b.bottom);
#endif

#ifdef MORPH_FEATURE_SCROLL
    out.scrollbarWidth = a.scrollbarWidth + (b.scrollbarWidth - a.scrollbarWidth) * t;
    for (int i = 0; i < 4; i++) {
        out.scrollbarTrackColor[i] = a.scrollbarTrackColor[i] + (b.scrollbarTrackColor[i] - a.scrollbarTrackColor[i]) * t;
        out.scrollbarThumbColor[i] = a.scrollbarThumbColor[i] + (b.scrollbarThumbColor[i] - a.scrollbarThumbColor[i]) * t;
    }
    out.scrollbarBorderRadius = a.scrollbarBorderRadius + (b.scrollbarBorderRadius - a.scrollbarBorderRadius) * t;
#endif

#ifdef MORPH_FEATURE_CURSOR
    out.cursor = b.cursor;
#endif
}

void MorphNode::updateAncestorHoverTransition(float dt) {
    if (m_ancestorHoverTransition && m_ancestorHoverTransition->active) {
        float dur = m_transitionDuration > 0.0f ? m_transitionDuration : 0.3f;
        m_ancestorHoverTransition->elapsed += dt;
        float t = m_ancestorHoverTransition->elapsed / dur;
        if (t >= 1.0f) {
            t = 1.0f;
            if (m_ancestorHoverTransition->applying) {
                style = m_ancestorHoverTransition->targetStyle;
            } else {
                style = m_ancestorHoverTransition->revertStyle;
            }
            m_ancestorHoverTransition->active = false;
        } else {
            const MorphStyle& from = m_ancestorHoverTransition->applying
                ? m_ancestorHoverTransition->revertStyle
                : m_ancestorHoverTransition->targetStyle;
            const MorphStyle& to = m_ancestorHoverTransition->applying
                ? m_ancestorHoverTransition->targetStyle
                : m_ancestorHoverTransition->revertStyle;
            interpolateStyles(style, from, to, applyEasing(t, m_transitionEasing));
        }
        markDirty(PaintDirty);
        markDirty(LayoutDirty);
    }
}

// ── Check if two styles differ in layout-affecting properties ──
static bool hasLayoutDiff(const MorphStyle& a, const MorphStyle& b) {
    if (memcmp(a.padding, b.padding, sizeof(float)*4) != 0) return true;
    if (memcmp(a.margin, b.margin, sizeof(float)*4) != 0) return true;
    if (a.explicitWidth != b.explicitWidth) return true;
    if (a.explicitHeight != b.explicitHeight) return true;
    if (a.minWidth != b.minWidth || a.maxWidth != b.maxWidth) return true;
    if (a.minHeight != b.minHeight || a.maxHeight != b.maxHeight) return true;
    if (a.fontSize != b.fontSize) return true;
#ifdef MORPH_FEATURE_BORDER
    if (a.borderWidth != b.borderWidth) return true;
#endif
#ifdef MORPH_FEATURE_FLEX
    if (a.gap != b.gap) return true;
#endif
#ifdef MORPH_FEATURE_POSITION
    if (a.left != b.left || a.right != b.right || a.top != b.top || a.bottom != b.bottom) return true;
#endif
    return false;
}

static bool isLayoutAnimProperty(AnimProperty p) {
    return p == AnimProperty::X || p == AnimProperty::Y
        || p == AnimProperty::W || p == AnimProperty::H;
}

void MorphNode::update(float dt) {
    bool wasTransitioning = m_isTransitioning;
    bool wasLayoutTransition = m_hasLayoutTransition;
    m_isTransitioning = false;
    m_hasLayoutTransition = false;

    updateHoverTransition(dt);
    updateAncestorHoverTransition(dt);
    updateAnimations(dt);

    if (m_hoverTransition && m_hoverTransition->active) {
        m_isTransitioning = true;
        if (hasLayoutDiff(m_hoverTransition->startStyle, m_hoverTransition->targetStyle))
            m_hasLayoutTransition = true;
    }
    if (m_ancestorHoverTransition && m_ancestorHoverTransition->active) {
        m_isTransitioning = true;
        if (hasLayoutDiff(m_ancestorHoverTransition->revertStyle, m_ancestorHoverTransition->targetStyle))
            m_hasLayoutTransition = true;
    }
    for (auto& a : m_animations) {
        if (a.running && !a.finished) {
            m_isTransitioning = true;
            if (isLayoutAnimProperty(a.property))
                m_hasLayoutTransition = true;
        }
    }

    for (auto* c : children) c->update(dt);

    if (m_isTransitioning != wasTransitioning) markDirty(PaintDirty);
    if (m_hasLayoutTransition != wasLayoutTransition) markDirty(PaintDirty);
}

// ── Display list helpers ─────────────────────────────────────
void MorphNode::recordDisplayList(Renderer& r) {
    // Base: nothing to record. Widgets override this.
}

void MorphNode::executeDisplayList(Renderer& r) {
    draw(r);
}

// ── Helpers for style string → FlatRenderNode enum conversion ─
static uint8_t overflowToEnum(const std::string& s) {
    if (s == "hidden") return 1;
    if (s == "scroll") return 2;
    if (s == "auto")   return 3;
    return 0; // visible
}
static uint8_t boxSizingToEnum(const std::string& s) {
    return (s == "border-box") ? 1 : 0;
}
static uint8_t displayToEnum(const std::string& s) {
    if (s == "flex")  return 1;
    if (s == "none")  return 2;
    if (s == "inline") return 3;
    return 0; // block
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

// ── Default: no extra data to flatten ────────────────────────
int MorphNode::flattenExtra(RenderFrame& frame, FlatRenderNode& fn) {
    (void)frame; (void)fn;
    return 0;
}

// ── Flatten node tree into lock-free render frame ─────────────
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

    // Copy display list into frame's flat drawOps array
    int dlStart = (int)frame.drawOps.size();
    frame.drawOps.insert(frame.drawOps.end(), m_displayList.begin(), m_displayList.end());
    fn.dlOffset = dlStart;
    fn.dlCount = (int)m_displayList.size();

    // Collect node-specific extra data (text ops, etc.)
    fn.textOpOffset = (int)frame.textOps.size();
    fn.textOpCount = flattenExtra(frame, fn);

    // Animations (only compositor-safe properties)
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
            default: continue; // W, H — not compositor-safe, skip
        }
        // Effective start time: now minus already-elapsed time
        double startTime = nowSec - (double)a.elapsed;
        frame.animations.push_back({idx, cap, a.from, a.to,
                                    startTime, a.duration, (uint8_t)a.easing, true});
    }

    frame.nodes.push_back(fn);

    // Flatten children
    for (auto* child : children) {
        int childIdx = child->flatten(frame, idx);
        frame.nodes[idx].children.push_back(childIdx);
    }

    return idx;
}

// ── W3C box-model helpers ────────────────────────────────────
//  Horizontal sizing bonus: content-box → pl+pr+bw*2, border-box → 0.
static float hBonus(const MorphStyle& s) {
#ifdef MORPH_FEATURE_BORDER_BOX
    if (s.boxSizing == "border-box") return 0.0f;
#endif
    float pl = s.padding[3], pr = s.padding[1];
#ifdef MORPH_FEATURE_BORDER
    return pl + pr + s.borderWidth * 2.0f;
#else
    return pl + pr;
#endif
}

//  Vertical sizing bonus: content-box → pt+pb+bw*2, border-box → 0.
static float vBonus(const MorphStyle& s) {
#ifdef MORPH_FEATURE_BORDER_BOX
    if (s.boxSizing == "border-box") return 0.0f;
#endif
    float pt = s.padding[0], pb = s.padding[2];
#ifdef MORPH_FEATURE_BORDER
    return pt + pb + s.borderWidth * 2.0f;
#else
    return pt + pb;
#endif
}

// ────────────────────────────────────────────────────────────────────────────
//   Main layout entry: two-pass width⤵ → height⤴ with feature-gating
// ────────────────────────────────────────────────────────────────────────────
void MorphNode::layout(float px, float py, float parentW, float parentH,
                       Renderer* r) {
    float ml = style.margin[3], mr = style.margin[1];
    float mt = style.margin[0], mb = style.margin[2];
    bool autoL = style.marginAuto[3], autoR = style.marginAuto[1];
    bool autoT = style.marginAuto[0], autoB = style.marginAuto[2];

    float pl = style.padding[3], pr = style.padding[1];
    float pt = style.padding[0], pb = style.padding[2];

#ifdef MORPH_FEATURE_BORDER
    float bw = style.borderWidth;
#else
    float bw = 0.0f;
#endif

    // ── Resolve auto margins ──
    // Treat auto as 0 for width computation, then resolve afterward
    float mlForWidth = autoL ? 0.0f : ml;
    float mrForWidth = autoR ? 0.0f : mr;

    // ── Width (border-box total) ──
    if (style.explicitWidth >= 0.0f) {
#ifdef MORPH_FEATURE_BORDER_BOX
        if (style.boxSizing == "border-box") {
            w = style.explicitWidth;
        } else
#endif
        {
            w = style.explicitWidth + pl + pr + bw * 2.0f;
        }
    } else {
        w = parentW - mlForWidth - mrForWidth;
    }
    if (w < 0.0f) w = 0.0f;

    // Resolve auto horizontal margins
    float availH = parentW - w;
    if (autoL && autoR) {
        ml = mr = fmaxf(availH * 0.5f, 0.0f);
    } else if (autoL) {
        ml = fmaxf(availH - mr, 0.0f);
    } else if (autoR) {
        mr = fmaxf(availH - ml, 0.0f);
    }
    // Auto top/bottom → 0 in normal flow
    if (autoT) mt = 0.0f;
    if (autoB) mb = 0.0f;
    // Store resolved margins separately for inspection.
    // style.margin stays as-declared (sentinels for auto) so
    // downstream code (e.g. flex layout reads child margins
    // after layout) sees the declared values, not the resolved ones.
    m_computedMargin[3] = ml; m_computedMargin[1] = mr;
    m_computedMargin[0] = mt; m_computedMargin[2] = mb;

    x = px + ml;
    y = py + mt;

    // ── Height (border-box total) ──
    if (style.explicitHeight >= 0.0f) {
#ifdef MORPH_FEATURE_BORDER_BOX
        if (style.boxSizing == "border-box") {
            h = style.explicitHeight;
        } else
#endif
        {
            h = style.explicitHeight + pt + pb + bw * 2.0f;
        }
    } else {
        h = 0.0f;  // auto-height below
    }

#ifdef MORPH_FEATURE_MIN_MAX
    if (style.minWidth > 0.0f && w < style.minWidth) w = style.minWidth;
    if (style.maxWidth > 0.0f && w > style.maxWidth) w = style.maxWidth;
    if (style.minHeight > 0.0f && h < style.minHeight) h = style.minHeight;
    if (style.maxHeight > 0.0f && h > style.maxHeight) h = style.maxHeight;
#endif

    // ── Content area (inside padding + border) ──
    float cw = w - pl - pr - bw * 2.0f;
    if (cw < 0.0f) cw = 0.0f;
    float ch = h - pt - pb - bw * 2.0f;
    if (ch < 0.0f) ch = 0.0f;
    float cx = x + bw + pl;
    float cy = y + bw + pt;

#ifdef MORPH_FEATURE_DISPLAY_NONE
    if (style.display == "none") {
        w = 0.0f; h = 0.0f;
        for (auto* c : children)
            c->layout(0.0f, 0.0f, 0.0f, 0.0f, r);
        contentH = 0.0f;
        scrollEnabled = false;
        return;
    }
#endif

    // Separate children by position & display
    std::vector<MorphNode*> normal;
    std::vector<MorphNode*> absChildren;
    for (auto* c : children) {
#ifdef MORPH_FEATURE_POSITION
        if (c->style.position == "absolute") {
            absChildren.push_back(c);
            continue;
        }
#endif
#ifdef MORPH_FEATURE_DISPLAY_NONE
        if (c->style.display == "none") {
            c->layout(0.0f, 0.0f, 0.0f, 0.0f, r);
            continue;
        }
#endif
        normal.push_back(c);
    }

    float maxBottom = cy;
    float maxRight  = 0.0f;

#ifdef MORPH_FEATURE_FLEX
    bool isRow = (style.display == "flex" && style.flexDirection == "row");
    bool isCol = !isRow;
#else
    bool isRow = false;
    bool isCol = true;
#endif
    int count = (int)normal.size();

#ifdef MORPH_FEATURE_FLEX
    // ── Flex layout (multi-line with wrap/grow/shrink) ─────────
    if (style.display == "flex") {
        struct FlexItem { MorphNode* node; float main, cross, mt, mr, mb, ml; };
        std::vector<FlexItem> items;

        // Pass 1: measure each child
        for (auto* c : normal) {
            c->layout(0.0f, 0.0f, cw, 0.0f, r);

            if (isRow && c->style.explicitWidth < 0.0f) {
                float cwVal = c->contentWidth(r);
                if (cwVal > 0.0f) c->w = cwVal;
            }

            float cmt = c->style.margin[0], cmb = c->style.margin[2];
            float cml = c->style.margin[3], cmr = c->style.margin[1];
            float childMain = isCol ? (c->h + cmt + cmb) : (c->w + cml + cmr);
            float childCross = isCol ? (c->w + cml + cmr) : (c->h + cmt + cmb);
            items.push_back({c, childMain, childCross, cmt, cmr, cmb, cml});
        }

        // Wrap into flex lines
        float mainAvail = isCol ? ch : cw;
        bool flexWrap = style.flexWrap == "wrap";

        struct FlexLine { std::vector<FlexItem*> fItems; float crossSize = 0.0f; float totalMain = 0.0f; };
        std::vector<FlexLine> lines;
        FlexLine curLine;

        for (auto& item : items) {
            if (flexWrap && !curLine.fItems.empty() && curLine.totalMain + item.main > mainAvail) {
                lines.push_back(curLine);
                curLine = FlexLine();
            }
            curLine.fItems.push_back(&item);
            curLine.totalMain += item.main + (curLine.fItems.size() > 1 ? style.gap : 0.0f);
            if (item.cross > curLine.crossSize) curLine.crossSize = item.cross;
        }
        if (!curLine.fItems.empty()) lines.push_back(curLine);
        if (lines.empty()) lines.push_back(FlexLine());

        // Apply flex-grow / flex-shrink per line
        for (auto& line : lines) {
            float extraGap = line.fItems.size() > 1 ? style.gap * (line.fItems.size() - 1) : 0.0f;
            float remaining = mainAvail - line.totalMain;

            if (remaining > 0.0f) {
                float growTotal = 0.0f;
                for (auto* item : line.fItems) growTotal += item->node->style.flexGrow;
                if (growTotal > 0.0f) {
                    float perUnit = remaining / growTotal;
                    for (auto* item : line.fItems) {
                        float g = item->node->style.flexGrow;
                        if (g > 0.0f) {
                            float add = perUnit * g;
                            if (isRow) item->node->w += add;
                            else item->node->h += add;
                            item->main += add;
                        }
                    }
                }
            }

            if (remaining < 0.0f) {
                float scaledTotal = 0.0f;
                for (auto* item : line.fItems) scaledTotal += item->main * item->node->style.flexShrink;
                if (scaledTotal > 0.0f) {
                    float toReduce = -remaining;
                    for (auto* item : line.fItems) {
                        float scaled = item->main * item->node->style.flexShrink;
                        float reduction = toReduce * scaled / scaledTotal;
                        float reduced = std::max(0.0f, item->main - reduction);
                        if (isRow) item->node->w -= item->main - reduced;
                        else item->node->h -= item->main - reduced;
                        item->main = reduced;
                    }
                }
            }
        }

        // Position each line
        float mainStart = isCol ? cy : cx;
        float crossStart = isCol ? cx : cy;
        float crossSize = isCol ? cw : ch;
        float cursorCross = crossStart;

        for (auto& line : lines) {
            float lineCross = line.crossSize;
            float extraGap = line.fItems.size() > 1 ? style.gap * (line.fItems.size() - 1) : 0.0f;
            float free = mainAvail - line.totalMain;

            // justify-content
            float offset = 0.0f;
            float itemGap = style.gap;
            if (style.justifyContent == "center") {
                offset = free * 0.5f;
            } else if (style.justifyContent == "flex-end") {
                offset = free;
            } else if (style.justifyContent == "space-between") {
                offset = 0.0f;
                itemGap = (line.fItems.size() > 1) ? style.gap + free / (line.fItems.size() - 1) : 0.0f;
            } else if (style.justifyContent == "space-around") {
                offset = line.fItems.size() > 0 ? free / (line.fItems.size() * 2) : 0.0f;
                itemGap = line.fItems.size() > 0 ? style.gap + free / line.fItems.size() : 0.0f;
            }

            float cursor = mainStart + offset;

            for (size_t i = 0; i < line.fItems.size(); i++) {
                auto* ci = line.fItems[i];
                float childMain = isCol ? ci->node->h : ci->node->w;
                float crossDim  = isCol ? ci->node->w : ci->node->h;

                float posMain = cursor + (isCol ? ci->mt : ci->ml);
                float posCross = cursorCross + (isCol ? ci->ml : ci->mt);

                if (lineCross > crossDim) {
                    if (style.alignItems == "center") {
                        float marginCross = isCol ? (ci->ml + ci->mr) : (ci->mt + ci->mb);
                        posCross = cursorCross + (isCol ? ci->ml : ci->mt) + (lineCross - (crossDim + marginCross)) * 0.5f;
                    } else if (style.alignItems == "flex-end") {
                        posCross = cursorCross + lineCross - crossDim;
                        posCross -= (isCol ? ci->mr : ci->mb);
                    }
                }

                float childX = isCol ? posCross : posMain;
                float childY = isCol ? posMain : posCross;
                float childPW = isCol ? ((style.alignItems == "stretch") ? crossSize : crossDim) : childMain;
                float childPH = isCol ? childMain : ((style.alignItems == "stretch") ? lineCross : crossDim);

                // Content-based sizing for non-stretch flex children
                if (style.alignItems != "stretch" && ci->node->style.explicitWidth < 0.0f && isCol) {
                    float cwVal = ci->node->contentWidth(r);
                    if (cwVal > 0.0f && cwVal < childPW) {
                        crossDim = cwVal;
                        childPW = cwVal;
                        if (lineCross > crossDim) {
                            if (style.alignItems == "center")
                                posCross = cursorCross + (lineCross - crossDim) * 0.5f;
                            else if (style.alignItems == "flex-end")
                                posCross = cursorCross + lineCross - crossDim;
                            childX = isCol ? posCross : posMain;
                            childY = isCol ? posMain : posCross;
                        }
                    }
                }

                float savedCM[4] = {
                    ci->node->m_computedMargin[0], ci->node->m_computedMargin[1],
                    ci->node->m_computedMargin[2], ci->node->m_computedMargin[3]
                };
                ci->node->layout(childX, childY, childPW, childPH, r);
                ci->node->m_computedMargin[0] = savedCM[0];
                ci->node->m_computedMargin[1] = savedCM[1];
                ci->node->m_computedMargin[2] = savedCM[2];
                ci->node->m_computedMargin[3] = savedCM[3];

                // Stretch alignment: item fills the cross-axis line size
                if (style.alignItems == "stretch" && ci->node->style.explicitWidth < 0.0f && isCol) {
                    float availW = lineCross - ci->ml - ci->mr;
                    if (availW < 0.0f) availW = 0.0f;
                    if (availW > ci->node->w) ci->node->w = availW;
                }
                if (style.alignItems == "stretch" && ci->node->style.explicitHeight < 0.0f && isRow) {
                    float availH = lineCross - ci->mt - ci->mb;
                    if (availH < 0.0f) availH = 0.0f;
                    if (availH > ci->node->h) ci->node->h = availH;
                }

                float outerH = ci->node->h;
                float outerW = ci->node->w;
                cursor += (isCol ? outerH + ci->mt + ci->mb : outerW + ci->ml + ci->mr) + itemGap;
                float cb = ci->node->y + outerH + ci->mb;
                if (cb > maxBottom) maxBottom = cb;
                if (isRow) {
                    float rb = ci->node->x + outerW + ci->mr;
                    if (rb > maxRight) maxRight = rb;
                }
            }

            cursorCross += lineCross + style.gap;
        }
        goto after_children;
    }
#endif // MORPH_FEATURE_FLEX

    // ────────────────────────────────────────────────────────────
    //   Block / Inline layout (non-flex) — order-preserving
    // ────────────────────────────────────────────────────────────
    {
        float curY = cy;
#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
        float prevMb = 0.0f;
#endif
#ifdef MORPH_FEATURE_INLINE
        std::vector<MorphNode*> currentInline;

        // Lambda to flush accumulated inline group in document order
        auto flushInline = [&]() {
            if (currentInline.empty()) return;

            // Measure all inline children
            struct InlineItem { MorphNode* node; float w, h; };
            std::vector<InlineItem> items;
            for (auto* c : currentInline) {
                c->layout(0.0f, 0.0f, cw, 0.0f, r);
                float iw = 0.0f;
                if (c->style.explicitWidth >= 0.0f) {
                    // layout() already set the correct total width
                    iw = c->w;
                } else if (c->contentWidth(r) > 0.0f) {
                    iw = c->contentWidth(r);
                }
                if (iw <= 0.0f) iw = cw;
                float ih = (c->h > 0.0f) ? c->h : (c->style.fontSize * 1.4f);
                items.push_back({c, iw, ih});
            }

            // Wrap into lines
            float lineX = cx;
            float lineY = curY;
            float lineH = 0.0f;
            size_t lineStart = 0;

            auto positionItems = [&](size_t end) {
                float alignX = cx;
                float lineW = lineX - cx;
                if (lineStart > 0 || currentInline[0]->style.textAlign == "center" || currentInline[0]->style.textAlign == "right") {
                    if (currentInline[0]->style.textAlign == "center")
                        alignX = cx + (cw - lineW) * 0.5f;
                    else if (currentInline[0]->style.textAlign == "right")
                        alignX = cx + cw - lineW;
                }
                float itemX = alignX;
                for (size_t j = lineStart; j < end; j++) {
                    auto& p = items[j];
                    float pml = p.node->style.margin[3];
                    p.node->x = itemX + pml;
                    p.node->y = lineY;
                    // Clamp width to available line space so TextNode doesn't
                    // keep the single-line contentWidth after wrapping
                    p.node->w = (p.w < cw) ? p.w : cw;
                    p.node->h = p.h;
                    for (auto* child : p.node->children) {
                        float cBw = 0.0f;
#ifdef MORPH_FEATURE_BORDER
                        cBw = p.node->style.borderWidth;
#endif
                        child->x = p.node->x + cBw + p.node->style.padding[3]
                                 + child->style.margin[3];
                        child->y = p.node->y + cBw + p.node->style.padding[0]
                                 + child->style.margin[0];
                        float childW = p.node->w
                                     - cBw * 2.0f
                                     - p.node->style.padding[3]
                                     - p.node->style.padding[1]
                                     - child->style.margin[3]
                                     - child->style.margin[1];
                        if (childW < 0) childW = 0;
                        child->w = childW;
                    }
                    itemX += pml + p.w + p.node->style.margin[1];
                }
            };

            for (size_t i = 0; i < items.size(); i++) {
                auto& it = items[i];
                float ml = it.node->style.margin[3];
                float mr = it.node->style.margin[1];
                float need = ml + it.w + mr;

                if (i > lineStart && lineX + need > cx + cw) {
                    positionItems(i);
                    lineY += lineH;
                    lineX = cx;
                    lineH = 0.0f;
                    lineStart = i;
                }

                lineX += need;
                if (it.h > lineH) lineH = it.h;
            }

            // Flush last line
            positionItems(items.size());
            float groupBottom = lineY + lineH;
            if (groupBottom > maxBottom) maxBottom = groupBottom;
            curY = groupBottom;
            prevMb = 0.0f;

            currentInline.clear();
        };

        // ── Single pass over children preserving document order ──
        for (auto* c : normal) {
            if (c->style.display == "inline" || c->type == "__text__") {
                currentInline.push_back(c);
            } else {
                // Flush any pending inline group before this block
                flushInline();

                // Layout block child with margin collapsing
                float cmt = c->style.margin[0];
                float cmb = c->style.margin[2];

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
                float collapsedMt = (prevMb > cmt) ? prevMb : cmt;
                float py = (curY - prevMb) + collapsedMt - cmt;
                c->layout(cx, py, cw, ch, r);
                prevMb = cmb;
#else
                c->layout(cx, curY + cmt, cw, ch, r);
#endif

                curY = c->y + c->h + cmb;
                float bottom = c->y + c->h + cmb;
                if (bottom > maxBottom) maxBottom = bottom;
            }
        }

        // Flush final inline group
        flushInline();

#else  // !MORPH_FEATURE_INLINE
        // ── Simple block-only layout ──
        for (auto* c : normal) {
            float cmt = c->style.margin[0];
            float cmb = c->style.margin[2];

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
            float collapsedMt = (prevMb > cmt) ? prevMb : cmt;
            float py = (curY - prevMb) + collapsedMt - cmt;
            c->layout(cx, py, cw, ch, r);
            prevMb = cmb;
#else
            c->layout(cx, curY + cmt, cw, ch, r);
#endif

            curY = c->y + c->h + cmb;
            float bottom = c->y + c->h + cmb;
            if (bottom > maxBottom) maxBottom = bottom;
        }
#endif // MORPH_FEATURE_INLINE
    }

after_children:

    // ── Layout absolute children ───────────────────────
    for (auto* c : absChildren) {
        float aw = c->style.explicitWidth >= 0.0f
#ifdef MORPH_FEATURE_BORDER_BOX
                   ? (c->style.boxSizing == "border-box"
                      ? c->style.explicitWidth
                      : c->style.explicitWidth + hBonus(c->style))
                   : 0.0f;
#else
                   ? c->style.explicitWidth + hBonus(c->style) : 0.0f;
#endif
        float ah = c->style.explicitHeight >= 0.0f
#ifdef MORPH_FEATURE_BORDER_BOX
                   ? (c->style.boxSizing == "border-box"
                      ? c->style.explicitHeight
                      : c->style.explicitHeight + vBonus(c->style))
                   : 0.0f;
#else
                   ? c->style.explicitHeight + vBonus(c->style) : 0.0f;
#endif
#ifdef MORPH_FEATURE_POSITION
        if (c->style.left > -1e8f && c->style.right > -1e8f)
            aw = cw - c->style.left - c->style.right;
        if (c->style.top > -1e8f && c->style.bottom > -1e8f)
            ah = ch - c->style.top - c->style.bottom;
#endif
        c->w = aw;
        c->h = ah;

#ifdef MORPH_FEATURE_POSITION
        float ax = cx + (c->style.left > -1e8f ? c->style.left : 0.0f);
        if (c->style.left <= -1e8f && c->style.right > -1e8f)
            ax = cx + cw - aw - c->style.right;
        float ay = cy + (c->style.top > -1e8f ? c->style.top : 0.0f);
        if (c->style.top <= -1e8f && c->style.bottom > -1e8f)
            ay = cy + ch - ah - c->style.bottom;
#else
        float ax = cx;
        float ay = cy;
#endif
        c->x = ax + c->style.margin[3];
        c->y = ay + c->style.margin[0];
        c->layout(ax, ay, aw, ah, r);
    }

    // ── Auto-height ──
    if (style.explicitHeight < 0.0f) {
        float autoH = (maxBottom - cy) + pt + pb + bw * 2.0f;
        if (autoH < 0.0f) autoH = 0.0f;
        if (autoH > h) h = autoH;
    }

    // ── Auto-width in row mode ──
#ifdef MORPH_FEATURE_FLEX
    if (style.display == "flex" && style.explicitWidth < 0.0f && isRow && maxRight > cx + cw) {
        float autoW = maxRight - x + pr + bw;
        if (autoW > w) w = autoW;
    }
#endif

    // ── Post-layout min/max height clamp ──
#ifdef MORPH_FEATURE_MIN_MAX
    if (style.minHeight > 0.0f && h < style.minHeight) h = style.minHeight;
    if (style.maxHeight > 0.0f && h > style.maxHeight) h = style.maxHeight;
#endif

    // Clamp auto-height to parent viewport when overflow is auto/scroll
    if (style.explicitHeight < 0.0f &&
        (style.overflow == "auto" || style.overflow == "scroll") &&
        parentH > 0.0f && h > parentH) {
        h = parentH;
    }

    // Compute scroll state
    contentH = maxBottom - cy + pt + pb + bw * 2.0f;
    if (contentH < h) contentH = h;
    scrollEnabled = (style.overflow == "scroll") ||
                    (style.overflow == "auto" && contentH > h);
    if (scrollEnabled) {
        if (scrollY > contentH - h) scrollY = contentH - h;
        if (scrollY < 0) scrollY = 0;
    }

    // Clear dirty flags so subsequent layoutIfNeeded calls skip this node.
    // Flex/inline/absolute paths call layout() directly without going through
    // layoutIfNeeded — without this, children remain LayoutDirty and get a
    // third re-layout (with wrong parentW) from layoutIfNeeded's iterator.
    clearDirty(LayoutDirty);
    clearDirty(StyleDirty);
    // When layout() is called directly (not via layoutIfNeeded), mark PaintDirty
    // so the display list gets regenerated with the updated position/size.
    markDirty(PaintDirty);
}

float MorphNode::contentWidth(Renderer* r) {
    float pl = style.padding[3], pr = style.padding[1];
#ifdef MORPH_FEATURE_BORDER
    float bw = style.borderWidth;
#else
    float bw = 0.0f;
#endif

    // With explicit width, return the total border-box width
    if (style.explicitWidth >= 0.0f) {
#ifdef MORPH_FEATURE_BORDER_BOX
        if (style.boxSizing == "border-box") {
            return style.explicitWidth;
        }
#endif
        return style.explicitWidth + pl + pr + bw * 2.0f;
    }

    // Row-mode flex: sum of child content widths + margins + gap
#ifdef MORPH_FEATURE_FLEX
    if (style.display == "flex" && style.flexDirection == "row") {
        float total = 0.0f;
        int count = 0;
        for (auto* c : children) {
            float cw = c->contentWidth(r);
            if (cw < 0.0f) return -1.0f;
            float cml = c->style.margin[3], cmr = c->style.margin[1];
            total += cw + cml + cmr;
            count++;
        }
        if (count > 1) total += (count - 1) * style.gap;
        return total + pl + pr + bw * 2.0f;
    }
#endif

    // Inline children: sum of widths of a single line (no wrapping)
#ifdef MORPH_FEATURE_INLINE
    {
        float totalInline = 0.0f;
        for (auto* c : children) {
            if (c->style.display == "inline" || c->type == "__text__") {
                float cw = c->contentWidth(r);
                if (cw > 0.0f)
                    totalInline += cw + c->style.margin[3] + c->style.margin[1];
            }
        }
        if (totalInline > 0.0f) {
            return totalInline + pl + pr + bw * 2.0f;
        }
    }
#endif

    // Block children: widest content (default)
    float maxCW = -1.0f;
    for (auto* c : children) {
#ifdef MORPH_FEATURE_DISPLAY_NONE
        if (c->style.display == "none") continue;
#endif
        float cw = c->contentWidth(r);
        if (cw > maxCW) maxCW = cw;
    }
    if (maxCW > -0.5f) {
        return maxCW + pl + pr + bw * 2.0f;
    }
    return -1.0f;
}

MorphNode* MorphNode::hitTest(float ex, float ey) {
    if (ex < x || ex > x + w || ey < y || ey > y + h) return nullptr;
    for (auto it = children.rbegin(); it != children.rend(); ++it) {
        auto* c = *it;
        float cy = c->y - (scrollEnabled ? scrollY : 0);
        if (ex >= c->x && ex <= c->x + c->w &&
            ey >= cy && ey <= cy + c->h) {
            auto* found = c->hitTest(ex, ey + (scrollEnabled ? scrollY : 0));
            if (found) return found;
        }
    }
    return this;
}

bool MorphNode::dispatchEvent(MorphEvent& e, float ex, float ey) {
    bool inBounds = (ex >= x && ex <= x + w && ey >= y && ey <= y + h);

#ifdef MORPH_FEATURE_SCROLL
    if (scrollEnabled && e.type == EventType::Scroll) {
        if (inBounds) {
            float oldScrollY = scrollY;
            scrollY -= e.scroll * 40.0f;
            if (scrollY < 0) scrollY = 0;
            if (scrollY > contentH - h) scrollY = contentH - h;
            if (scrollY != oldScrollY) {
                markDirty(PaintDirty);
                for (auto* c : children) c->markDirty(PaintDirty);
            }
            return true;
        }
    }

    if (scrollEnabled && inBounds) {
        float sw = style.scrollbarWidth;
        float trackX = x + w - sw;
        bool onScrollbar = (ex >= trackX && ex <= trackX + sw);
        if (onScrollbar && (e.type == EventType::MouseDown || e.type == EventType::Click)) {
            float thumbH = (h / contentH) * h;
            float thumbY = y + (scrollY / (contentH - h)) * (h - thumbH);
            if (ey >= thumbY && ey <= thumbY + thumbH) {
                scrollDragging = true;
                scrollDragStartY = ey;
                scrollDragStartVal = scrollY;
                return true;
            } else {
                float page = h * 0.7f;
                float oldScrollY = scrollY;
                scrollY += (ey < thumbY) ? -page : page;
                if (scrollY < 0) scrollY = 0;
                if (scrollY > contentH - h) scrollY = contentH - h;
                if (scrollY != oldScrollY) {
                    markDirty(PaintDirty);
                    for (auto* c : children) c->markDirty(PaintDirty);
                }
                return true;
            }
        }
        if (e.type == EventType::MouseUp) {
            scrollDragging = false;
        }
        if (e.type == EventType::MouseMove && scrollDragging) {
            float oldScrollY = scrollY;
            float thumbH = (h / contentH) * h;
            float dy = ey - scrollDragStartY;
            float range = contentH - h;
            float thumbRange = h - thumbH;
            if (thumbRange > 0) {
                scrollY = scrollDragStartVal + (dy / thumbRange) * range;
                if (scrollY < 0) scrollY = 0;
                if (scrollY > range) scrollY = range;
            }
            if (scrollY != oldScrollY) {
                markDirty(PaintDirty);
                for (auto* c : children) c->markDirty(PaintDirty);
            }
            return true;
        }
    }
#endif

    for (auto it = children.rbegin(); it != children.rend(); ++it) {
        auto* c = *it;
        float cy = c->y - (scrollEnabled ? scrollY : 0);
        if (ex >= c->x && ex <= c->x + c->w &&
            ey >= cy && ey <= cy + c->h) {
#ifdef MORPH_FEATURE_SCROLL
            if (scrollEnabled && (cy + c->h <= y || cy >= y + h))
                continue;
#endif
            if (c->dispatchEvent(e, ex, ey + (scrollEnabled ? scrollY : 0)))
                return true;
        }
    }
    return onEvent(e);
}
