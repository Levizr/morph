#include "../node.h"
#include "../renderer.h"
#include <cmath>
#include <cstring>

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

#ifdef MORPH_FEATURE_POSITION
// Shift a sticky node and every descendant so children stay glued to it.
static void shiftStickySubtree(MorphNode* n, float dx, float dy) {
    n->x += dx;
    n->y += dy;
    for (auto* c : n->children) shiftStickySubtree(c, dx, dy);
}

static void markSubtreePaintDirty(MorphNode* n) {
    n->markDirty(PaintDirty);
    for (auto* c : n->children) markSubtreePaintDirty(c);
}

// `position: sticky` — keeps its normal-flow box (m_flowX/m_flowY) but gets
// clamped against the nearest scroll container's scrollport, between the
// top/bottom (and left/right) offsets and its containing block.
void MorphNode::applySticky() {
    MorphNode* sc = nearestScrollContainer();
    if (!sc) return;

    float newX = m_flowX;
    float newY = m_flowY;

#ifdef MORPH_FEATURE_BORDER
    float bw = sc->style.borderWidth;
#else
    float bw = 0.0f;
#endif

    if (style.left > -1e8f || style.right > -1e8f) {
        float spLeft = sc->x + bw + sc->style.padding[3];
        float spW = sc->w - 2.0f * bw - sc->style.padding[3] - sc->style.padding[1];
        if (spW < 0.0f) spW = 0.0f;
        if (style.left > -1e8f) {
            float minX = spLeft + style.left;
            if (newX < minX) newX = minX;
        }
        if (style.right > -1e8f) {
            float maxX = spLeft + spW - style.right - w;
            if (newX > maxX) newX = maxX;
        }
        if (parent) {
            float cbLeft = parent->x + bw + parent->style.padding[3];
            float cbW = parent->w - 2.0f * bw - parent->style.padding[3] - parent->style.padding[1];
            if (cbW < 0.0f) cbW = 0.0f;
            if (newX < cbLeft) newX = cbLeft;
            float cbRight = cbLeft + cbW - w;
            if (newX > cbRight) newX = cbRight;
        }
    }

    if (style.top > -1e8f || style.bottom > -1e8f) {
        float spTop = sc->y + bw + sc->style.padding[0];
        float spH = sc->h - 2.0f * bw - sc->style.padding[0] - sc->style.padding[2];
        if (spH < 0.0f) spH = 0.0f;
        if (style.top > -1e8f) {
            float minY = spTop + style.top + sc->scrollY;
            if (newY < minY) newY = minY;
        }
        if (style.bottom > -1e8f) {
            float maxY = spTop + spH - style.bottom - h + sc->scrollY;
            if (newY > maxY) newY = maxY;
        }
        if (parent) {
            float cbTop = parent->x + bw + parent->style.padding[0];
            float cbH = parent->h - 2.0f * bw - parent->style.padding[0] - parent->style.padding[2];
            if (cbH < 0.0f) cbH = 0.0f;
            if (newY < cbTop) newY = cbTop;
            float cbBottom = cbTop + cbH - h;
            if (newY > cbBottom) newY = cbBottom;
        }
    }

    if (newX != x || newY != y) {
        shiftStickySubtree(this, newX - x, newY - y);
        markSubtreePaintDirty(this);
    }
}

void MorphNode::updateStickySubtree() {
    if (style.position == "sticky") applySticky();
    for (auto* c : children) c->updateStickySubtree();
}
#endif

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

#ifdef MORPH_FEATURE_POSITION
    // Root node: establish the viewport + initial containing block.
    if (!parent) {
        m_winW = parentW;
        m_winH = parentH;
        m_absCbX = 0.0f; m_absCbY = 0.0f;
        m_absCbW = parentW; m_absCbH = parentH;
    }
#endif

#ifdef MORPH_FEATURE_POSITION
    bool isAbs = (style.position == "absolute" || style.position == "fixed");
    bool isRel = (style.position == "relative" || style.position == "sticky");

    if (isAbs) {
        // ── Out of flow: absolute (nearest positioned ancestor's padding box)
        //    or fixed (viewport). px/py/parentW/parentH are ignored here.
        float cbx, cby, cbw, cbh;
        if (style.position == "fixed") {
            cbx = 0.0f; cby = 0.0f;
            cbw = m_winW; cbh = m_winH;
        } else {
            cbx = m_absCbX; cby = m_absCbY;
            cbw = m_absCbW; cbh = m_absCbH;
        }

        // Width
        if (style.explicitWidth >= 0.0f) {
            w = style.explicitWidth;
#ifdef MORPH_FEATURE_BORDER_BOX
            if (style.boxSizing != "border-box")
#endif
                w += pl + pr + bw * 2.0f;
        } else {
            w = -1.0f;
        }
        if (style.left > -1e8f && style.right > -1e8f)
            w = cbw - style.left - style.right;
        if (w < 0.0f) w = 0.0f;
        if (style.explicitWidth < 0.0f) {
            // Auto width → shrink-to-fit (content-based), capped by available.
            float avail = cbw;
            if (style.left > -1e8f) avail -= style.left;
            if (style.right > -1e8f) avail -= style.right;
            if (style.left > -1e8f && style.right > -1e8f) avail = cbw - style.left - style.right;
            float sw = r ? contentWidth(r) : 0.0f;
            w = (sw > 0.0f && sw < avail) ? sw : avail;
            if (w < 0.0f) w = 0.0f;
        }

        // Height
        if (style.explicitHeight >= 0.0f) {
            h = style.explicitHeight;
#ifdef MORPH_FEATURE_BORDER_BOX
            if (style.boxSizing != "border-box")
#endif
                h += pt + pb + bw * 2.0f;
        } else {
            h = 0.0f;
        }
        if (style.top > -1e8f && style.bottom > -1e8f)
            h = cbh - style.top - style.bottom;
        if (h < 0.0f) h = 0.0f;

        // Position
        x = cbx + (style.left > -1e8f ? style.left : 0.0f);
        if (style.left <= -1e8f && style.right > -1e8f)
            x = cbx + cbw - w - style.right;
        y = cby + (style.top > -1e8f ? style.top : 0.0f);
        if (style.top <= -1e8f && style.bottom > -1e8f)
            y = cby + cbh - h - style.bottom;
        x += ml;
        y += mt;
    } else
#endif
    {
    float mlForWidth = autoL ? 0.0f : ml;
    float mrForWidth = autoR ? 0.0f : mr;

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

    float availH = parentW - w;
    if (autoL && autoR) {
        ml = mr = fmaxf(availH * 0.5f, 0.0f);
    } else if (autoL) {
    ml = fmaxf(availH - mr, 0.0f);
    } else if (autoR) {
    mr = fmaxf(availH - ml, 0.0f);
    }
    if (autoT) mt = 0.0f;
    if (autoB) mb = 0.0f;
    if (getenv("MORPH_LAYOUT_DEBUG") && (mt != 0.0f || mb != 0.0f)) {
        printf("[layout()] type=%s px=%.2f py=%.2f mt=%.2f mb=%.2f -> y=%.2f\n",
               type.c_str(), px, py, mt, mb, py + mt);
    }
    m_computedMargin[3] = ml; m_computedMargin[1] = mr;
    m_computedMargin[0] = mt; m_computedMargin[2] = mb;

    x = px + ml;
    y = py + mt;

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
        h = 0.0f;
    }

#ifdef MORPH_FEATURE_POSITION
    // ── Relative: offset the flow box without affecting siblings. ──
    // Sticky skips the fixed offset here — its offset is the scroll clamp
    // applied in applySticky() (called below), anchored at m_flowX/m_flowY.
    if (style.position == "relative") {
        float offX = 0.0f, offY = 0.0f;
        if (style.left > -1e8f) offX = style.left;
        else if (style.right > -1e8f) offX = -style.right;
        if (style.top > -1e8f) offY = style.top;
        else if (style.bottom > -1e8f) offY = -style.bottom;
        x += offX;
        y += offY;
    }
#endif
    }

#ifdef MORPH_FEATURE_POSITION
    m_flowX = x;
    m_flowY = y;
    if (style.position == "sticky")
        applySticky();
#endif

#ifdef MORPH_FEATURE_MIN_MAX
    if (style.minWidth > 0.0f && w < style.minWidth) w = style.minWidth;
    if (style.maxWidth > 0.0f && w > style.maxWidth) w = style.maxWidth;
    if (style.minHeight > 0.0f && h < style.minHeight) h = style.minHeight;
    if (style.maxHeight > 0.0f && h > style.maxHeight) h = style.maxHeight;
#endif

    float cw = w - pl - pr - bw * 2.0f;
    if (cw < 0.0f) cw = 0.0f;
    float ch = h - pt - pb - bw * 2.0f;
    if (ch < 0.0f) ch = 0.0f;
    float cx = x + bw + pl;
    float cy = y + bw + pt;

#ifdef MORPH_FEATURE_POSITION
    // Containing block for absolute descendants = padding box of the nearest
    // positioned ancestor (this node if positioned, otherwise inherited).
    float cbX, cbY, cbW, cbH;
    if (isPositioned()) {
        cbX = x + bw; cbY = y + bw;
        cbW = w - 2.0f * bw; cbH = h - 2.0f * bw;
        if (cbW < 0.0f) cbW = 0.0f;
        if (cbH < 0.0f) cbH = 0.0f;
    } else {
        cbX = m_absCbX; cbY = m_absCbY;
        cbW = m_absCbW; cbH = m_absCbH;
    }
    for (auto* c : children) {
        c->m_absCbX = cbX; c->m_absCbY = cbY;
        c->m_absCbW = cbW; c->m_absCbH = cbH;
        c->m_winW = m_winW; c->m_winH = m_winH;
    }
#endif

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

    std::vector<MorphNode*> normal;
    std::vector<MorphNode*> absChildren;
    std::vector<MorphNode*> fixedChildren;
    for (auto* c : children) {
#ifdef MORPH_FEATURE_POSITION
        if (c->style.position == "absolute") {
            absChildren.push_back(c);
            continue;
        }
        if (c->style.position == "fixed") {
            fixedChildren.push_back(c);
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

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
    // Parent–child margin-collapse tracking: a boundary-less block parent
    // lets its first block child's top margin and its last block child's
    // bottom margin collapse through — they are excluded from our height and
    // passed up via m_computedMargin for the parent to apply.
    // (Declared before the flex `goto` so the jump doesn't cross them.)
    bool inlineBeforeFirstBlock = false;
    bool inlineAfterLastBlock = false;
    bool firstBlockChild = false;
    bool lastBlockChildMbSet = false;
    float firstChildMtEff = 0.0f;
    float lastChildMbEff = 0.0f;
#endif

#ifdef MORPH_FEATURE_FLEX
    bool isRow = (style.display == "flex" && style.flexDirection == "row");
    bool isCol = !isRow;
#else
    bool isRow = false;
    bool isCol = true;
#endif
    int count = (int)normal.size();

#ifdef MORPH_FEATURE_FLEX
    if (style.display == "flex") {
        struct FlexItem { MorphNode* node; float main, cross, mt, mr, mb, ml; };
        std::vector<FlexItem> items;

        for (auto* c : normal) {
            if (c->isWhitespaceOnly()) continue;
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

        float mainStart = isCol ? cy : cx;
        float crossStart = isCol ? cx : cy;
        float crossSize = isCol ? cw : ch;
        float cursorCross = crossStart;

        for (auto& line : lines) {
            float lineCross = line.crossSize;
            float extraGap = line.fItems.size() > 1 ? style.gap * (line.fItems.size() - 1) : 0.0f;
            float free = mainAvail - line.totalMain;

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
#endif

    {
        float curY = cy;
#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
        float prevMb = 0.0f;
#endif
#ifdef MORPH_FEATURE_INLINE
        std::vector<MorphNode*> currentInline;

        auto flushInline = [&]() {
            if (currentInline.empty()) return;

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
            if (!firstBlockChild) inlineBeforeFirstBlock = true;
            else inlineAfterLastBlock = true;
#endif
            struct InlineItem { MorphNode* node; float w, h; bool ws; };
            std::vector<InlineItem> items;
            for (auto* c : currentInline) {
                c->layout(0.0f, 0.0f, cw, 0.0f, r);
                float iw = 0.0f;
                if (c->style.explicitWidth >= 0.0f) {
                    iw = c->w;
                } else if (c->contentWidth(r) > 0.0f) {
                    iw = c->contentWidth(r);
                }
                if (iw <= 0.0f) iw = cw;
                float ih = (c->h > 0.0f) ? c->h : (c->style.fontSize * 1.4f);
                items.push_back({c, iw, ih, c->isWhitespaceOnly()});
            }

            // Whitespace-only text (newlines/indent between elements) collapses
            // like a browser: a single space between inline items, nothing at
            // the start/end of a line, and no line box when it's all whitespace.
            int firstVis = -1, lastVis = -1;
            for (size_t i = 0; i < items.size(); i++) {
                if (!items[i].ws) {
                    if (firstVis < 0) firstVis = (int)i;
                    lastVis = (int)i;
                }
            }
            if (firstVis < 0) {
                for (auto* c : currentInline) {
                    c->w = 0.0f;
                    c->h = 0.0f;
                }
                currentInline.clear();
#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
                prevMb = 0.0f;
#endif
                return;
            }
            for (size_t i = 0; i < (size_t)firstVis; i++) {
                items[i].w = 0.0f;
                items[i].h = 0.0f;
            }
            for (size_t i = (size_t)lastVis + 1; i < items.size(); i++) {
                items[i].w = 0.0f;
                items[i].h = 0.0f;
            }
            for (size_t i = (size_t)firstVis; i <= (size_t)lastVis; i++) {
                if (items[i].ws) {
                    items[i].w = r ? r->measureTextWidth(" ", items[i].node->style.fontSize, "normal") : 4.0f;
                    items[i].h = 0.0f;
                }
            }

            float lineX = cx;
            float lineY = curY;
            float lineH = 0.0f;
            size_t lineStart = 0;

            auto positionItems = [&](size_t end) {
                float alignX = cx;
                float lineW = lineX - cx;
                // Line alignment comes from THIS container's text-align,
                // not from the first inline item (e.g. a button with
                // text-align:center must not center the whole line).
                if (lineStart > 0 || style.textAlign == "center" || style.textAlign == "right") {
                    if (style.textAlign == "center")
                        alignX = cx + (cw - lineW) * 0.5f;
                    else if (style.textAlign == "right")
                        alignX = cx + cw - lineW;
                }
                float itemX = alignX;
                for (size_t j = lineStart; j < end; j++) {
                    auto& p = items[j];
                    float pml = p.node->style.margin[3];
                    p.node->x = itemX + pml;
                    p.node->y = lineY;
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
                    if (items[i].ws) {
                        items[i].w = 0.0f;
                        items[i].h = 0.0f;
                    }
                }

                lineX += need;
                if (it.h > lineH) lineH = it.h;
            }

            positionItems(items.size());
            float groupBottom = lineY + lineH;
            if (groupBottom > maxBottom) maxBottom = groupBottom;
            curY = groupBottom;
#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
            prevMb = 0.0f;
#endif
            currentInline.clear();
        };

        for (auto* c : normal) {
            if (c->style.display == "inline" || c->style.display == "inline-block"
                || c->type == "__text__") {
                currentInline.push_back(c);
            } else {
                flushInline();

                float ownMt = c->style.margin[0];
                float ownMb = c->style.margin[2];

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
                if (getenv("MORPH_LAYOUT_DEBUG")) {
                    printf("[layout] %s child type=%s y=%.2f curY=%.2f cy=%.2f ownMt=%.2f firstBlock=%d inlineBefore=%d pt=%g bw=%g\n",
                           style.display.c_str(), c->type.c_str(), c->y, curY, cy, ownMt,
                           firstBlockChild ? 1 : 0, inlineBeforeFirstBlock ? 1 : 0, pt, bw);
                }
                // A child's collapsed-through margins (m_computedMargin) are
                // only known after its own layout pass, so lay it out once at
                // a provisional y to learn them, then move it to its final y
                // and relayout only if the position changed.  This keeps the
                // very first layout pass correct (no stale-margin pass 1).
                float provY = (!firstBlockChild && !inlineBeforeFirstBlock
                               && pt == 0.0f && bw == 0.0f)
                                  ? curY - ownMt
                                  : curY;
                c->layout(cx, provY, cw, ch, r);

                // Effective margins include margins collapsed up from the
                // child's own children (stored in m_computedMargin by the
                // child's layout pass just above).
                float passMt = c->m_computedMargin[0];
                float passMb = c->m_computedMargin[2];
                float cmt = (passMt > ownMt) ? passMt : ownMt;
                float cmb = (passMb > ownMb) ? passMb : ownMb;
                float collapsedMt = (prevMb > cmt) ? prevMb : cmt;
                // The first block child of a parent with no top boundary
                // collapses its top margin with ours: apply nothing inside —
                // the margin is passed up to our own parent instead.
                float py;
                if (!firstBlockChild && !inlineBeforeFirstBlock
                    && pt == 0.0f && bw == 0.0f)
                    py = curY - ownMt;
                else
                    py = (curY - prevMb) + collapsedMt - ownMt;
                if (py != provY)
                    c->layout(cx, py, cw, ch, r);
                prevMb = cmb;
#else
                float cmt = ownMt;
                float cmb = ownMb;
                c->layout(cx, curY + cmt, cw, ch, r);
#endif

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
                if (!firstBlockChild) {
                    firstBlockChild = true;
                    if (!inlineBeforeFirstBlock && pt == 0.0f && bw == 0.0f)
                        firstChildMtEff = (passMt > ownMt) ? passMt : ownMt;
                }
                lastChildMbEff = (passMb > ownMb) ? passMb : ownMb;
                lastBlockChildMbSet = true;
                inlineAfterLastBlock = false;
#endif

                curY = c->y + c->h + cmb;
                float bottom = c->y + c->h + cmb;
                if (bottom > maxBottom) maxBottom = bottom;
            }
        }

        flushInline();

#else
        for (auto* c : normal) {
            float ownMt = c->style.margin[0];
            float ownMb = c->style.margin[2];

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
            // Provisional first pass to learn the child's collapsed-through
            // margins before deciding its final y (see the inline path above).
            float provY = (!firstBlockChild && pt == 0.0f && bw == 0.0f)
                              ? curY - ownMt
                              : curY;
            c->layout(cx, provY, cw, ch, r);

            float passMt = c->m_computedMargin[0];
            float passMb = c->m_computedMargin[2];
            float cmt = (passMt > ownMt) ? passMt : ownMt;
            float cmb = (passMb > ownMb) ? passMb : ownMb;
            float collapsedMt = (prevMb > cmt) ? prevMb : cmt;
            float py;
            if (!firstBlockChild && pt == 0.0f && bw == 0.0f)
                py = curY - ownMt;
            else
                py = (curY - prevMb) + collapsedMt - ownMt;
            if (py != provY)
                c->layout(cx, py, cw, ch, r);
            prevMb = cmb;
#else
            float cmt = ownMt;
            float cmb = ownMb;
            c->layout(cx, curY + cmt, cw, ch, r);
#endif

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
            if (!firstBlockChild) {
                firstBlockChild = true;
                if (pt == 0.0f && bw == 0.0f)
                    firstChildMtEff = (passMt > ownMt) ? passMt : ownMt;
            }
            lastChildMbEff = (passMb > ownMb) ? passMb : ownMb;
            lastBlockChildMbSet = true;
#endif

            curY = c->y + c->h + cmb;
            float bottom = c->y + c->h + cmb;
            if (bottom > maxBottom) maxBottom = bottom;
        }
#endif
    }

after_children:

#ifdef MORPH_FEATURE_POSITION
    // Absolute children position themselves relative to their containing
    // block (m_absCb*, resolved inside layout) — out of flow, so they don't
    // affect this node's height.
    for (auto* c : absChildren)
        c->layout(0.0f, 0.0f, 0.0f, 0.0f, r);
    // Fixed children are positioned relative to the viewport.
    for (auto* c : fixedChildren)
        c->layout(0.0f, 0.0f, 0.0f, 0.0f, r);
#endif

    if (style.explicitHeight < 0.0f) {
        float autoH = (maxBottom - cy) + pt + pb + bw * 2.0f;
#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
        // Parent–child margin collapse: the last block child's bottom margin
        // collapses through a boundary-less parent, so it must not inflate
        // our height — our parent applies it as the gap after us instead.
        if (pb == 0.0f && bw == 0.0f && lastBlockChildMbSet && !inlineAfterLastBlock)
            autoH -= lastChildMbEff;
#endif
        if (autoH < 0.0f) autoH = 0.0f;
        if (autoH > h) h = autoH;
    }

#ifdef MORPH_FEATURE_FLEX
    if (style.display == "flex" && style.explicitWidth < 0.0f && isRow && maxRight > cx + cw) {
        float autoW = maxRight - x + pr + bw;
        if (autoW > w) w = autoW;
    }
#endif

#ifdef MORPH_FEATURE_MIN_MAX
    if (style.minHeight > 0.0f && h < style.minHeight) h = style.minHeight;
    if (style.maxHeight > 0.0f && h > style.maxHeight) h = style.maxHeight;
#endif

    if (style.explicitHeight < 0.0f &&
        (style.overflow == "auto" || style.overflow == "scroll") &&
        parentH > 0.0f && h > parentH) {
        h = parentH;
    }

    contentH = maxBottom - cy + pt + pb + bw * 2.0f;
#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
    if (pb == 0.0f && bw == 0.0f && lastBlockChildMbSet && !inlineAfterLastBlock)
        contentH -= lastChildMbEff;
#endif
    if (contentH < h) contentH = h;

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
    // Pass collapsed-through margins up to our parent (parent–child margin
    // collapse), e.g. an h1's 21px margins escape a boundary-less div that
    // wraps it and become the gap around that div.
    if (style.display != "flex" && style.explicitHeight < 0.0f) {
        if (pt == 0.0f && bw == 0.0f && firstBlockChild && !inlineBeforeFirstBlock
            && firstChildMtEff > m_computedMargin[0])
            m_computedMargin[0] = firstChildMtEff;
        if (pb == 0.0f && bw == 0.0f && lastBlockChildMbSet && !inlineAfterLastBlock
            && lastChildMbEff > m_computedMargin[2])
            m_computedMargin[2] = lastChildMbEff;
    }
#endif
    // Browsers vertically center button content. Buttons (tag type=="button")
    // are laid out as plain block containers in the IR, so on their own the
    // text stays pinned to the top. When the button is taller than its
    // content (fixed height), shift the flow children down so the label sits
    // centered. Flex buttons are left alone — flexbox handles their layout.
    if (type == "button" && style.display != "flex")
    {
        float btnContentH = h - pt - pb - bw * 2.0f;
        if (btnContentH > 0.0f)
        {
            float childTop = cy, childBottom = cy;
            bool any = false;
            for (auto* c : children)
            {
#ifdef MORPH_FEATURE_POSITION
                if (c->style.position == "absolute" || c->style.position == "fixed") continue;
#endif
                if (c->isWhitespaceOnly()) continue;
                float top = c->y, bottom = c->y + c->h;
                if (!any) { childTop = top; childBottom = bottom; any = true; }
                else { if (top < childTop) childTop = top; if (bottom > childBottom) childBottom = bottom; }
            }
            if (any) {
                float offset = (btnContentH - (childBottom - childTop)) * 0.5f;
                if (offset > 0.0f)
                    for (auto* c : children) {
#ifdef MORPH_FEATURE_POSITION
                        if (c->style.position == "absolute" || c->style.position == "fixed") continue;
#endif
                        if (c->isWhitespaceOnly()) continue;
                        c->y += offset;
                    }
            }
        }
    }

    scrollEnabled = (style.overflow == "scroll") ||
                    (style.overflow == "auto" && contentH > h);
    if (scrollEnabled) {
        if (scrollY > contentH - h) scrollY = contentH - h;
        if (scrollY < 0) scrollY = 0;
#ifdef MORPH_FEATURE_POSITION
        // Sticky descendants are laid out before scrollEnabled was known, so
        // resolve their clamps now that the scrollport geometry is final.
        updateStickySubtree();
#endif
    }

    clearDirty(LayoutDirty);
    clearDirty(StyleDirty);
#ifdef MORPH_FEATURE_DEV
    // Dev: paint dirtiness is established by the geometry diff after the
    // layout pass (window.cpp syncPaintDirtyTree), not blanket here.
#else
    markDirty(PaintDirty);
#endif
}
