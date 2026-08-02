#include "../node.h"
#include "../renderer.h"
#include <cmath>
#include <cstring>

static float applyEasing(float t, Easing e) {
    switch (e) {
        case Easing::Linear:   return t;
        case Easing::EaseIn:   return t * t;
        case Easing::EaseOut:  return 1.0f - (1.0f - t) * (1.0f - t);
        case Easing::EaseInOut: return t < 0.5f ? 2.0f * t * t : 1.0f - (float)pow(-2.0f * t + 2.0f, 2.0f) / 2.0f;
    }
    return t;
}

static bool hasLayoutDiff(const MorphStyle& a, const MorphStyle& b);

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
            float val = a.to;
            setAnimProperty(this, a.property, val);
        }
    }
}

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
    if (delta.padding[0] != 0.0f || delta.padding[1] != 0.0f || delta.padding[2] != 0.0f || delta.padding[3] != 0.0f)
        memcpy(target.padding, delta.padding, sizeof(float)*4);
    if (delta.margin[0] != 0.0f || delta.margin[1] != 0.0f || delta.margin[2] != 0.0f || delta.margin[3] != 0.0f)
        memcpy(target.margin, delta.margin, sizeof(float)*4);
    if (delta.marginAuto[0] || delta.marginAuto[1] || delta.marginAuto[2] || delta.marginAuto[3])
        memcpy(target.marginAuto, delta.marginAuto, sizeof(bool)*4);
    if (delta.explicitWidth >= 0.0f)  target.explicitWidth = delta.explicitWidth;
    if (delta.explicitHeight >= 0.0f) target.explicitHeight = delta.explicitHeight;
    if (delta.minWidth >= 0.0f)       target.minWidth = delta.minWidth;
    if (delta.maxWidth >= 0.0f)       target.maxWidth = delta.maxWidth;
    if (delta.minHeight >= 0.0f)      target.minHeight = delta.minHeight;
    if (delta.maxHeight >= 0.0f)      target.maxHeight = delta.maxHeight;
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
#ifdef MORPH_FEATURE_POSITION
    if (delta.left > -1e8f)  target.left = delta.left;
    if (delta.right > -1e8f) target.right = delta.right;
    if (delta.top > -1e8f)   target.top = delta.top;
    if (delta.bottom > -1e8f) target.bottom = delta.bottom;
#endif
#ifdef MORPH_FEATURE_ZINDEX
    if (delta.zIndexSet) {
        target.zIndex = delta.zIndex;
        target.zIndexSet = true;
    }
#endif
#ifdef MORPH_FEATURE_CURSOR
    if (delta.cursor != "default") target.cursor = delta.cursor;
#endif
#ifdef MORPH_FEATURE_BORDER
    if (delta.borderWidth > 0.0f) target.borderWidth = delta.borderWidth;
    if (delta.borderColor[0] != 0.0f || delta.borderColor[1] != 0.0f || delta.borderColor[2] != 0.0f || delta.borderColor[3] != 1.0f)
        memcpy(target.borderColor, delta.borderColor, sizeof(float)*4);
    if (delta.borderStyle != "none") target.borderStyle = delta.borderStyle;
#endif
#ifdef MORPH_FEATURE_SCROLL
    if (delta.scrollbarWidth != 8.0f) target.scrollbarWidth = delta.scrollbarWidth;
    if (delta.scrollbarTrackColor[0] != 0.85f || delta.scrollbarTrackColor[1] != 0.85f || delta.scrollbarTrackColor[2] != 0.85f || delta.scrollbarTrackColor[3] != 0.4f)
        memcpy(target.scrollbarTrackColor, delta.scrollbarTrackColor, sizeof(float)*4);
    if (delta.scrollbarThumbColor[0] != 0.5f || delta.scrollbarThumbColor[1] != 0.5f || delta.scrollbarThumbColor[2] != 0.5f || delta.scrollbarThumbColor[3] != 0.6f)
        memcpy(target.scrollbarThumbColor, delta.scrollbarThumbColor, sizeof(float)*4);
    if (delta.scrollbarBorderRadius != 4.0f) target.scrollbarBorderRadius = delta.scrollbarBorderRadius;
#endif
}

#ifdef MORPH_FEATURE_ZINDEX
// A node's z-index only affects ordering among its parent's children, so a
// runtime change must invalidate the parent's cached paint order.
static void invalidatePaintOrderOnZ(MorphNode* n) {
    if (n && n->parent) n->parent->invalidatePaintOrder();
}
#endif

static void _applyStateStyle(MorphNode* node, bool state,
                             MorphStyle* stateStyle, HoverTransition*& trans) {
    if (!stateStyle) return;
    if (!trans)
        trans = new HoverTransition();
    MorphStyle layoutBefore = node->style;
    if (state) {
        if (!trans->active)
            trans->preHoverStyle = node->style;
        if (node->m_transitionDuration > 0.0f) {
            trans->startStyle = node->style;
            trans->targetStyle = node->style;
            applyStyleDelta(trans->targetStyle, *stateStyle);
            trans->elapsed = 0.0f;
            trans->active = true;
        } else {
            applyStyleDelta(node->style, *stateStyle);
        }
    } else {
        if (node->m_transitionDuration > 0.0f) {
            trans->startStyle = node->style;
            trans->targetStyle = trans->preHoverStyle;
            trans->elapsed = 0.0f;
            trans->active = true;
        } else {
            node->style = trans->preHoverStyle;
        }
    }
    node->markDirty(PaintDirty);
    if (hasLayoutDiff(layoutBefore, node->style))
        node->markDirty(LayoutDirty);
}

void MorphNode::onHover(bool state) {
    _applyStateStyle(this, state, hoverStyle, m_hoverTransition);
    _applyAncestorHover(state);
}

void MorphNode::onActive(bool state) {
    _applyStateStyle(this, state, activeStyle, m_activeTransition);
    _applyAncestorActive(state);
}

static void _applyOneAncestorRule(MorphNode* child, const AncestorHoverRule& rule,
                                  AncestorHoverTransition*& t, bool state) {
    if (state) {
        if (!t) {
            t = new AncestorHoverTransition();
        }
        if (t->applyCount == 0)
            t->revertStyle = child->style;
        t->applyCount++;

        MorphStyle layoutBefore = child->style;
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
        if (hasLayoutDiff(layoutBefore, child->style))
            child->markDirty(LayoutDirty);
    } else {
        if (!t) return;
        t->applyCount--;
        if (t->applyCount > 0) return;

        MorphStyle layoutBefore = child->style;
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
        if (hasLayoutDiff(layoutBefore, child->style))
            child->markDirty(LayoutDirty);
    }
}

void MorphNode::_applyAncestorHover(bool state) {
    for (auto* child : children) {
        child->_applyAncestorHover(state);
        for (auto& rule : child->m_ancestorHoverRules) {
            if (rule.ancestorTag.empty() || type == rule.ancestorTag) {
                _applyOneAncestorRule(child, rule, child->m_ancestorHoverTransition, state);
            }
        }
    }
}

void MorphNode::_applyAncestorActive(bool state) {
    for (auto* child : children) {
        child->_applyAncestorActive(state);
        for (auto& rule : child->m_ancestorActiveRules) {
            if (rule.ancestorTag.empty() || type == rule.ancestorTag) {
                _applyOneAncestorRule(child, rule, child->m_ancestorActiveTransition, state);
            }
        }
    }
}

static void _updateStateTransition(MorphNode* node, HoverTransition*& trans, float dt) {
    if (!trans || !trans->active) return;
    trans->elapsed += dt;
    float t = trans->elapsed / node->m_transitionDuration;
    if (t >= 1.0f) {
        t = 1.0f;
        node->style = trans->targetStyle;
        trans->active = false;
    } else {
        MorphNode::interpolateStyles(node->style, trans->startStyle,
                                     trans->targetStyle,
                                     applyEasing(t, node->m_transitionEasing));
    }
    node->markDirty(PaintDirty);
}

void MorphNode::updateHoverTransition(float dt) {
    _updateStateTransition(this, m_hoverTransition, dt);
}

void MorphNode::updateActiveTransition(float dt) {
    _updateStateTransition(this, m_activeTransition, dt);
}

void MorphNode::interpolateStyles(MorphStyle& out, const MorphStyle& a,
                                   const MorphStyle& b, float t) {
    for (int i = 0; i < 4; i++) {
        out.bgColor[i] = a.bgColor[i] + (b.bgColor[i] - a.bgColor[i]) * t;
        out.color[i] = a.color[i] + (b.color[i] - a.color[i]) * t;
        out.padding[i] = a.padding[i] + (b.padding[i] - a.padding[i]) * t;
        out.margin[i] = a.margin[i] + (b.margin[i] - a.margin[i]) * t;
        out.marginAuto[i] = b.marginAuto[i];
    }
    out.borderRadius = a.borderRadius + (b.borderRadius - a.borderRadius) * t;
    out.fontSize = a.fontSize + (b.fontSize - a.fontSize) * t;

    auto lerpIfSet = [t](float av, float bv) {
        return (av >= 0.0f && bv >= 0.0f) ? av + (bv - av) * t : bv;
    };
    out.explicitWidth = lerpIfSet(a.explicitWidth, b.explicitWidth);
    out.explicitHeight = lerpIfSet(a.explicitHeight, b.explicitHeight);
    out.minWidth = lerpIfSet(a.minWidth, b.minWidth);
    out.maxWidth = lerpIfSet(a.maxWidth, b.maxWidth);
    out.minHeight = lerpIfSet(a.minHeight, b.minHeight);
    out.maxHeight = lerpIfSet(a.maxHeight, b.maxHeight);

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
    out.borderStyle = b.borderStyle;
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

#ifdef MORPH_FEATURE_ZINDEX
    out.zIndex = b.zIndex;
    out.zIndexSet = b.zIndexSet;
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

static void _updateAncestorTransition(MorphNode* node, AncestorHoverTransition* t, float dt) {
    if (!t || !t->active) return;
    float dur = node->m_transitionDuration > 0.0f ? node->m_transitionDuration : 0.3f;
    t->elapsed += dt;
    float tt = t->elapsed / dur;
    if (tt >= 1.0f) {
        tt = 1.0f;
        if (t->applying) {
            node->style = t->targetStyle;
        } else {
            node->style = t->revertStyle;
        }
        t->active = false;
    } else {
        const MorphStyle& from = t->applying
            ? t->revertStyle
            : t->targetStyle;
        const MorphStyle& to = t->applying
            ? t->targetStyle
            : t->revertStyle;
        MorphNode::interpolateStyles(node->style, from, to, applyEasing(tt, node->m_transitionEasing));
    }
    node->markDirty(PaintDirty);
}

void MorphNode::updateAncestorHoverTransition(float dt) {
    _updateAncestorTransition(this, m_ancestorHoverTransition, dt);
}

void MorphNode::updateAncestorActiveTransition(float dt) {
    _updateAncestorTransition(this, m_ancestorActiveTransition, dt);
}

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
    updateActiveTransition(dt);
    updateAncestorHoverTransition(dt);
    updateAncestorActiveTransition(dt);
    updateAnimations(dt);

    if (m_hoverTransition && m_hoverTransition->active) {
        m_isTransitioning = true;
        if (hasLayoutDiff(m_hoverTransition->startStyle, m_hoverTransition->targetStyle))
            m_hasLayoutTransition = true;
    }
    if (m_activeTransition && m_activeTransition->active) {
        m_isTransitioning = true;
        if (hasLayoutDiff(m_activeTransition->startStyle, m_activeTransition->targetStyle))
            m_hasLayoutTransition = true;
    }
    if (m_ancestorHoverTransition && m_ancestorHoverTransition->active) {
        m_isTransitioning = true;
        if (hasLayoutDiff(m_ancestorHoverTransition->revertStyle, m_ancestorHoverTransition->targetStyle))
            m_hasLayoutTransition = true;
    }
    if (m_ancestorActiveTransition && m_ancestorActiveTransition->active) {
        m_isTransitioning = true;
        if (hasLayoutDiff(m_ancestorActiveTransition->revertStyle, m_ancestorActiveTransition->targetStyle))
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
