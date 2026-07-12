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
    if (style.display == "flex") {
        struct FlexItem { MorphNode* node; float main, cross, mt, mr, mb, ml; };
        std::vector<FlexItem> items;

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

            struct InlineItem { MorphNode* node; float w, h; };
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
                items.push_back({c, iw, ih});
            }

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
            if (c->style.display == "inline" || c->type == "__text__") {
                currentInline.push_back(c);
            } else {
                flushInline();

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

        flushInline();

#else
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
#endif
    }

after_children:

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

    if (style.explicitHeight < 0.0f) {
        float autoH = (maxBottom - cy) + pt + pb + bw * 2.0f;
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
    if (contentH < h) contentH = h;
    scrollEnabled = (style.overflow == "scroll") ||
                    (style.overflow == "auto" && contentH > h);
    if (scrollEnabled) {
        if (scrollY > contentH - h) scrollY = contentH - h;
        if (scrollY < 0) scrollY = 0;
    }

    clearDirty(LayoutDirty);
    clearDirty(StyleDirty);
    markDirty(PaintDirty);
}
