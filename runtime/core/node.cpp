#include "node.h"
#include "renderer.h"

#ifdef MORPH_FEATURE_BORDER
static bool borderAffectsLayout(const MorphStyle& s) {
    return s.borderWidth > 0.0f && s.borderStyle == "solid" && s.boxSizing != "border-box";
}
#endif

void MorphNode::layout(float px, float py, float parentW, float parentH,
                       Renderer* r) {
#ifdef MORPH_FEATURE_POSITION
    if (style.position == "absolute") {
        w = style.explicitWidth >= 0.0f ? style.explicitWidth : 0.0f;
        h = style.explicitHeight >= 0.0f ? style.explicitHeight : 0.0f;
    } else
#endif
    {
        float ml = style.margin[3], mr = style.margin[1];
        float mt = style.margin[0], mb = style.margin[2];
        x = px + ml;
        y = py + mt;
        w = style.explicitWidth >= 0.0f ? style.explicitWidth : parentW - ml - mr;
        h = style.explicitHeight >= 0.0f ? style.explicitHeight : 0.0f;
    }

    float pl = style.padding[3], pr = style.padding[1];
    float pt = style.padding[0], pb = style.padding[2];

    float cw = w - pl - pr;
    if (cw < 0) cw = 0;
    if (style.maxWidth > 0.0f && cw > style.maxWidth) cw = style.maxWidth;
    float ch = h - pt - pb;
    if (ch < 0) ch = 0;
    float cx = x + pl;
    float cy = y + pt;

    // Separate normal vs absolute children
    std::vector<MorphNode*> normal;
    std::vector<MorphNode*> absChildren;
    for (auto* c : children) {
#ifdef MORPH_FEATURE_POSITION
        if (c->style.position == "absolute")
            absChildren.push_back(c);
        else
#endif
            normal.push_back(c);
    }

    float maxBottom = 0.0f;
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
    // ── Pass 1: measure children (temp position 0,0) ──
    struct ChildInfo { MorphNode* node; float w, h, mt, mb, ml, mr; };
    std::vector<ChildInfo> info;
    float totalMain = 0.0f;

    for (auto* c : normal) {
        c->layout(0.0f, 0.0f, cw, 0.0f, r);

        // For row children without explicit width, prefer content-based width
        if (isRow && c->style.explicitWidth < 0.0f) {
            float cwVal = c->contentWidth(r);
            if (cwVal > 0.0f) c->w = cwVal;
        }

        float cmt = c->style.margin[0], cmb = c->style.margin[2];
        float cml = c->style.margin[3], cmr = c->style.margin[1];
        float childDim = isCol ? c->h : c->w;
#ifdef MORPH_FEATURE_BORDER
        if (borderAffectsLayout(c->style))
            childDim += c->style.borderWidth * 2.0f;
#endif
        totalMain += childDim + (isCol ? cmt + cmb : cml + cmr);
        info.push_back({c, c->w, c->h, cmt, cmb, cml, cmr});
    }

    float gapTotal = (count > 1) ? (count - 1) * style.gap : 0.0f;

    // ── Pass 2: position each child and re-layout at final position ──
    float mainStart = isCol ? cy : cx;
    float mainSize  = isCol ? ch : cw;
    float cross     = isCol ? cx : cy;
    float crossSize = isCol ? cw : ch;

    if (mainSize > totalMain + gapTotal) {
        if (style.justifyContent == "center") {
            mainStart += (mainSize - totalMain - gapTotal) * 0.5f;
        } else if (style.justifyContent == "flex-end") {
            mainStart += mainSize - totalMain - gapTotal;
        }
    }

    float cursor = mainStart;
    bool isFlex = (style.display == "flex");
    for (size_t i = 0; i < normal.size(); i++) {
        auto& ci = info[i];
        float childMain = isCol ? ci.h : ci.w;
        float crossDim  = isCol ? ci.w : ci.h;

        float posMain = cursor + (isCol ? ci.mt : ci.ml);
#ifdef MORPH_FEATURE_BORDER
        // Shift child forward by borderWidth so visual rect starts at cursor position
        // (border extends backward by bw from the content origin)
        if (borderAffectsLayout(ci.node->style))
            posMain += ci.node->style.borderWidth;
#endif
        float posCross = cross + (isCol ? ci.ml : ci.mt);

        if (isFlex && crossSize > crossDim) {
            if (style.alignItems == "center") {
                posCross = cross + (crossSize - crossDim) * 0.5f;
            } else if (style.alignItems == "flex-end") {
                posCross = cross + crossSize - crossDim;
                posCross -= (isCol ? ci.mr : ci.mb);
            }
        }

        float childX = isCol ? posCross : posMain;
        float childY = isCol ? posMain : posCross;

        float childPW, childPH;
        if (isCol) {
            childPW = (isFlex && style.alignItems == "stretch") ? cw : crossDim;
            childPH = childMain;
        } else {
            childPW = childMain;
            childPH = (isFlex && style.alignItems == "stretch") ? ch : crossDim;
        }

        // Content-based sizing for non-stretch flex children
        if (isFlex && style.alignItems != "stretch" && ci.node->style.explicitWidth < 0.0f) {
            if (isCol) {
                float cwVal = ci.node->contentWidth(r);
                if (cwVal > 0.0f && cwVal < childPW) {
                    crossDim = cwVal;
                    childPW = cwVal;
                    if (crossSize > crossDim) {
                        if (style.alignItems == "center") {
                            posCross = cross + (crossSize - crossDim) * 0.5f;
                        } else if (style.alignItems == "flex-end") {
                            posCross = cross + crossSize - crossDim;
                            posCross -= (isCol ? ci.mr : ci.mb);
                        }
                    }
                    childX = isCol ? posCross : posMain;
                    childY = isCol ? posMain : posCross;
                }
            }
        }

        ci.node->layout(childX, childY, childPW, childPH, r);

        // Stretch alignment: fill available cross dimension
        if (isFlex && style.alignItems == "stretch" && ci.node->style.explicitHeight < 0.0f) {
            if (isCol && cw > crossDim) {
#ifdef MORPH_FEATURE_BORDER
                if (borderAffectsLayout(ci.node->style))
                    ci.node->w = cw - ci.node->style.borderWidth * 2.0f;
                else
#endif
                    ci.node->w = cw;
            } else if (isRow && ch > crossDim) {
#ifdef MORPH_FEATURE_BORDER
                if (borderAffectsLayout(ci.node->style))
                    ci.node->h = ch - ci.node->style.borderWidth * 2.0f;
                else
#endif
                    ci.node->h = ch;
            }
        }

        float outerH = ci.node->h;
        float outerW = ci.node->w;
#ifdef MORPH_FEATURE_BORDER
        if (borderAffectsLayout(ci.node->style)) {
            outerH += ci.node->style.borderWidth * 2.0f;
            outerW += ci.node->style.borderWidth * 2.0f;
        }
#endif
        cursor += (isCol ? outerH + ci.mt + ci.mb : outerW + ci.ml + ci.mr) + style.gap;
        float cb = ci.node->y + outerH + ci.mb;
        if (cb > maxBottom) maxBottom = cb;
        if (isRow) {
            float rb = ci.node->x + outerW + ci.mr;
            if (rb > maxRight) maxRight = rb;
        }
    }
#else
    // ── Simple stack (no flex) ──
    float curY = cy;
    for (auto* c : normal) {
        c->layout(0.0f, 0.0f, cw, 0.0f, r);
        float cmt = c->style.margin[0], cmb = c->style.margin[2];
        c->y = curY + cmt;
#ifdef MORPH_FEATURE_BORDER
        if (borderAffectsLayout(c->style))
            c->y += c->style.borderWidth;
#endif
        c->x = cx + c->style.margin[3];
        float pw = cw;
        c->layout(c->x, c->y, pw, 0.0f, r);
        float outerH = c->h;
#ifdef MORPH_FEATURE_BORDER
        if (borderAffectsLayout(c->style))
            outerH += c->style.borderWidth * 2.0f;
#endif
        curY += outerH + cmt + cmb;
        float cb = c->y + outerH + cmb;
        if (cb > maxBottom) maxBottom = cb;
    }
#endif

    // ── Layout absolute children ───────────────────────
    for (auto* c : absChildren) {
        float aw = c->style.explicitWidth >= 0.0f ? c->style.explicitWidth : 0.0f;
        float ah = c->style.explicitHeight >= 0.0f ? c->style.explicitHeight : 0.0f;
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

    // Auto-height
    if (style.explicitHeight < 0.0f) {
        float autoH = maxBottom - y + pb;
        if (autoH < 0) autoH = 0;
        if (autoH > h) h = autoH;
    }

    // Auto-width in row mode
#ifdef MORPH_FEATURE_FLEX
    if (style.explicitWidth < 0.0f && isRow && maxRight > cx + cw) {
        float autoW = maxRight - x + pr;
        if (autoW > w) w = autoW;
    }
#endif

    // Clamp auto-height to parent viewport when overflow is auto/scroll
    if (style.explicitHeight < 0.0f &&
        (style.overflow == "auto" || style.overflow == "scroll") &&
        parentH > 0.0f && h > parentH) {
        h = parentH;
    }

    // Compute scroll state
    contentH = maxBottom - y + pb;
    if (contentH < h) contentH = h;
    scrollEnabled = (style.overflow == "scroll") ||
                    (style.overflow == "auto" && contentH > h);
    if (scrollEnabled) {
        if (scrollY > contentH - h) scrollY = contentH - h;
        if (scrollY < 0) scrollY = 0;
    }
}

float MorphNode::contentWidth(Renderer* r) {
    if (style.explicitWidth >= 0.0f) return style.explicitWidth;

#ifdef MORPH_FEATURE_FLEX
    bool isRow = (style.display == "flex" && style.flexDirection == "row");

    if (isRow) {
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
        float pl = style.padding[3], pr = style.padding[1];
        return total + pl + pr;
    }
#endif

    float maxCW = -1.0f;
    for (auto* c : children) {
        float cw = c->contentWidth(r);
        if (cw > maxCW) maxCW = cw;
    }
    if (maxCW > -0.5f) {
        float pl = style.padding[3], pr = style.padding[1];
        return maxCW + pl + pr;
    }
    return -1.0f;
}

MorphNode* MorphNode::hitTest(float ex, float ey) {
    float hx = x, hy = y, hw = w, hh = h;
#ifdef MORPH_FEATURE_BORDER
    if (borderAffectsLayout(style)) {
        hx -= style.borderWidth;
        hy -= style.borderWidth;
        hw += style.borderWidth * 2.0f;
        hh += style.borderWidth * 2.0f;
    }
#endif
    if (ex < hx || ex > hx + hw || ey < hy || ey > hy + hh) return nullptr;
    for (auto it = children.rbegin(); it != children.rend(); ++it) {
        auto* c = *it;
        float cy = c->y - (scrollEnabled ? scrollY : 0);
        float chx = c->x, chy = cy, chw = c->w, chh = c->h;
#ifdef MORPH_FEATURE_BORDER
        if (borderAffectsLayout(c->style)) {
            chx -= c->style.borderWidth;
            chy -= c->style.borderWidth;
            chw += c->style.borderWidth * 2.0f;
            chh += c->style.borderWidth * 2.0f;
        }
#endif
        if (ex >= chx && ex <= chx + chw &&
            ey >= chy && ey <= chy + chh) {
            auto* found = c->hitTest(ex, ey + (scrollEnabled ? scrollY : 0));
            if (found) return found;
        }
    }
    return this;
}

bool MorphNode::dispatchEvent(MorphEvent& e, float ex, float ey) {
    float hx = x, hy = y, hw = w, hh = h;
#ifdef MORPH_FEATURE_BORDER
    if (borderAffectsLayout(style)) {
        hx -= style.borderWidth;
        hy -= style.borderWidth;
        hw += style.borderWidth * 2.0f;
        hh += style.borderWidth * 2.0f;
    }
#endif
    bool inBounds = (ex >= hx && ex <= hx + hw && ey >= hy && ey <= hy + hh);

#ifdef MORPH_FEATURE_SCROLL
    if (scrollEnabled && e.type == EventType::Scroll) {
        if (inBounds) {
            scrollY -= e.scroll * 40.0f;
            if (scrollY < 0) scrollY = 0;
            if (scrollY > contentH - h) scrollY = contentH - h;
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
                scrollY += (ey < thumbY) ? -page : page;
                if (scrollY < 0) scrollY = 0;
                if (scrollY > contentH - h) scrollY = contentH - h;
                return true;
            }
        }
        if (e.type == EventType::MouseUp) {
            scrollDragging = false;
        }
        if (e.type == EventType::MouseMove && scrollDragging) {
            float thumbH = (h / contentH) * h;
            float dy = ey - scrollDragStartY;
            float range = contentH - h;
            float thumbRange = h - thumbH;
            if (thumbRange > 0) {
                scrollY = scrollDragStartVal + (dy / thumbRange) * range;
                if (scrollY < 0) scrollY = 0;
                if (scrollY > range) scrollY = range;
            }
            return true;
        }
    }
#endif

    for (auto it = children.rbegin(); it != children.rend(); ++it) {
        auto* c = *it;
        float cy = c->y - (scrollEnabled ? scrollY : 0);
        float chx = c->x, chy = cy, chw = c->w, chh = c->h;
#ifdef MORPH_FEATURE_BORDER
        if (borderAffectsLayout(c->style)) {
            chx -= c->style.borderWidth;
            chy -= c->style.borderWidth;
            chw += c->style.borderWidth * 2.0f;
            chh += c->style.borderWidth * 2.0f;
        }
#endif
        if (ex >= chx && ex <= chx + chw &&
            ey >= chy && ey <= chy + chh) {
#ifdef MORPH_FEATURE_SCROLL
            if (scrollEnabled && (chy + chh <= y || chy >= y + h))
                continue;
#endif
            if (c->dispatchEvent(e, ex, ey + (scrollEnabled ? scrollY : 0)))
                return true;
        }
    }
    return onEvent(e);
}
