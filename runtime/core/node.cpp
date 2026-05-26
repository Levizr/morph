#include "node.h"
#include "renderer.h"

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
    float pl = style.padding[3], pr = style.padding[1];
    float pt = style.padding[0], pb = style.padding[2];

#ifdef MORPH_FEATURE_BORDER
    float bw = style.borderWidth;
#else
    float bw = 0.0f;
#endif

    x = px + ml;
    y = py + mt;

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
        w = parentW - ml - mr;
    }
    if (w < 0.0f) w = 0.0f;

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
    // ── Flex layout (two-pass) ─────────────────────────────────
    if (style.display == "flex") {
        struct ChildInfo { MorphNode* node; float w, h, mt, mb, ml, mr; };
        std::vector<ChildInfo> info;
        float totalMain = 0.0f;

        // Pass 1: measure children
        for (auto* c : normal) {
            c->layout(0.0f, 0.0f, cw, 0.0f, r);

            if (isRow && c->style.explicitWidth < 0.0f) {
                float cwVal = c->contentWidth(r);
                if (cwVal > 0.0f) c->w = cwVal;
            }

            float cmt = c->style.margin[0], cmb = c->style.margin[2];
            float cml = c->style.margin[3], cmr = c->style.margin[1];
            float childDim = isCol ? c->h : c->w;
            totalMain += childDim + (isCol ? cmt + cmb : cml + cmr);
            info.push_back({c, c->w, c->h, cmt, cmb, cml, cmr});
        }

        float gapTotal = (count > 1) ? (count - 1) * style.gap : 0.0f;

        // Pass 2: position each child
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
        for (size_t i = 0; i < normal.size(); i++) {
            auto& ci = info[i];
            float childMain = isCol ? ci.h : ci.w;
            float crossDim  = isCol ? ci.w : ci.h;

            float posMain = cursor + (isCol ? ci.mt : ci.ml);
            float posCross = cross + (isCol ? ci.ml : ci.mt);

            if (crossSize > crossDim) {
                if (style.alignItems == "center") {
                    posCross = cross + (crossSize - crossDim) * 0.5f;
                } else if (style.alignItems == "flex-end") {
                    posCross = cross + crossSize - crossDim;
                    posCross -= (isCol ? ci.mr : ci.mb);
                }
            }

            float childX = isCol ? posCross : posMain;
            float childY = isCol ? posMain : posCross;
            float childPW = isCol ? ((style.alignItems == "stretch") ? cw : crossDim) : childMain;
            float childPH = isCol ? childMain : ((style.alignItems == "stretch") ? ch : crossDim);

            // Content-based sizing for non-stretch flex children
            if (style.alignItems != "stretch" && ci.node->style.explicitWidth < 0.0f && isCol) {
                float cwVal = ci.node->contentWidth(r);
                if (cwVal > 0.0f && cwVal < childPW) {
                    crossDim = cwVal;
                    childPW = cwVal;
                    if (crossSize > crossDim) {
                        if (style.alignItems == "center")
                            posCross = cross + (crossSize - crossDim) * 0.5f;
                        else if (style.alignItems == "flex-end")
                            posCross = cross + crossSize - crossDim;
                        childX = isCol ? posCross : posMain;
                        childY = isCol ? posMain : posCross;
                    }
                }
            }

            ci.node->layout(childX, childY, childPW, childPH, r);

            // Stretch alignment: item fills the cross-axis line size
            if (style.alignItems == "stretch" && ci.node->style.explicitWidth < 0.0f && isCol) {
                float availW = cw - ci.ml - ci.mr;
                if (availW < 0.0f) availW = 0.0f;
                if (availW > ci.node->w)
                    ci.node->w = availW;
            }
            if (style.alignItems == "stretch" && ci.node->style.explicitHeight < 0.0f && isRow) {
                float availH = ch - ci.mt - ci.mb;
                if (availH < 0.0f) availH = 0.0f;
                if (availH > ci.node->h)
                    ci.node->h = availH;
            }

            float outerH = ci.node->h;
            float outerW = ci.node->w;
            cursor += (isCol ? outerH + ci.mt + ci.mb : outerW + ci.ml + ci.mr) + style.gap;
            float cb = ci.node->y + outerH + ci.mb;
            if (cb > maxBottom) maxBottom = cb;
            if (isRow) {
                float rb = ci.node->x + outerW + ci.mr;
                if (rb > maxRight) maxRight = rb;
            }
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
                    p.node->w = p.w;
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
            if (c->style.display == "inline") {
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
            if (c->style.display == "inline") {
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
