#pragma once
#include <vector>
#include <functional>
#include <string>
#include "event.h"

struct MorphStyle {
    float bgColor[4] = {0,0,0,0};
    float color[4]   = {0,0,0,1};
    float borderRadius = 0.0f;
    float fontSize     = 16.0f;
    float padding[4]   = {0,0,0,0};
    float margin[4]    = {0,0,0,0};
    float gap          = 0.0f;
    float explicitWidth  = -1.0f;
    float explicitHeight = -1.0f;
    std::string fontWeight = "normal";
    std::string overflow = "visible";
    float scrollbarWidth  = 8.0f;
    float scrollbarTrackColor[4] = {0.85f, 0.85f, 0.85f, 0.4f};
    float scrollbarThumbColor[4] = {0.5f, 0.5f, 0.5f, 0.6f};
    float scrollbarBorderRadius = 4.0f;

    // Position
    std::string position = "static";
    float left   = -1e9f;
    float right  = -1e9f;
    float top    = -1e9f;
    float bottom = -1e9f;

    // Flex
    std::string display = "block";
    std::string flexDirection = "column";
    std::string justifyContent = "flex-start";
    std::string alignItems = "stretch";
    std::string flexWrap = "nowrap";

    // Text
    std::string textAlign = "left";
    float maxWidth = -1.0f;

    // Interaction
    std::string cursor = "default";
};

class Renderer;

class MorphNode {
public:
    float x = 0, y = 0, w = 0, h = 0;
    MorphStyle style;
    MorphNode* parent = nullptr;
    std::vector<MorphNode*> children;
    bool focused = false;

    // Scroll state
    float scrollY = 0;
    float contentH = 0;
    bool scrollEnabled = false;
    bool scrollThumbHover = false;
    bool scrollDragging = false;
    float scrollDragStartY = 0;
    float scrollDragStartVal = 0;

    virtual void layout(float px, float py, float parentW, float parentH,
                        Renderer* r = nullptr) {
        if (style.position == "absolute") {
            w = style.explicitWidth >= 0.0f ? style.explicitWidth : 0.0f;
            h = style.explicitHeight >= 0.0f ? style.explicitHeight : 0.0f;
            // Position will be set by parent — just compute children below
        } else {
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
            if (c->style.position == "absolute")
                absChildren.push_back(c);
            else
                normal.push_back(c);
        }

        // Layout normal-flow children
        float maxBottom = 0.0f;
        float maxRight  = 0.0f;
        bool isRow = (style.display == "flex" && style.flexDirection == "row");
        bool isCol = !isRow;
        int count = (int)normal.size();

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
            totalMain += (isCol ? c->h : c->w) + (isCol ? cmt + cmb : cml + cmr);
            info.push_back({c, c->w, c->h, cmt, cmb, cml, cmr});
        }

        float gapTotal = (count > 1) ? (count - 1) * style.gap : 0.0f;

        // ── Pass 2: position each child and re-layout at final position ──
        float mainStart = isCol ? cy : cx;
        float mainSize  = isCol ? ch : cw;
        float cross     = isCol ? cx : cy;
        float crossSize = isCol ? cw : ch;

        // Only apply justifyContent when container has room (avoids negative offset on auto-sized containers)
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
            float posCross = cross + (isCol ? ci.ml : ci.mt);

            // Cross-axis alignment (only when there's extra space)
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

            // Parent dimensions to pass to child's layout
            float childPW, childPH;
            if (isCol) {
                childPW = (isFlex && style.alignItems == "stretch") ? cw : crossDim;
                childPH = childMain;
            } else {
                childPW = childMain;
                childPH = (isFlex && style.alignItems == "stretch") ? ch : crossDim;
            }

            // For non-stretch flex children without explicit cross size,
            // use content-based sizing so center/flex-end alignment has room
            if (isFlex && style.alignItems != "stretch" && ci.node->style.explicitWidth < 0.0f) {
                if (isCol) {
                    float cwVal = ci.node->contentWidth(r);
                    if (cwVal > 0.0f && cwVal < childPW) {
                        crossDim = cwVal;
                        childPW = cwVal;
                        // Recompute cross-axis position with new narrower width
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

            // Apply stretch on cross axis (layout doesn't use parentH for auto-height)
            if (isFlex && style.alignItems == "stretch" && ci.node->style.explicitHeight < 0.0f) {
                if (isCol && cw > crossDim) {
                    ci.node->w = cw;
                } else if (isRow && ch > crossDim) {
                    ci.node->h = ch;
                }
            }

            float actualH = ci.node->h;
            float actualW = ci.node->w;
            cursor += (isCol ? actualH + ci.mt + ci.mb : actualW + ci.ml + ci.mr) + style.gap;
            float cb = ci.node->y + actualH + ci.mb;
            if (cb > maxBottom) maxBottom = cb;
            if (isRow) {
                float rb = ci.node->x + actualW + ci.mr;
                if (rb > maxRight) maxRight = rb;
            }
        }

        // ── Layout absolute children ───────────────────────
        for (auto* c : absChildren) {
            float aw = c->style.explicitWidth >= 0.0f ? c->style.explicitWidth : 0.0f;
            float ah = c->style.explicitHeight >= 0.0f ? c->style.explicitHeight : 0.0f;
            if (c->style.left > -1e8f && c->style.right > -1e8f)
                aw = cw - c->style.left - c->style.right;
            if (c->style.top > -1e8f && c->style.bottom > -1e8f)
                ah = ch - c->style.top - c->style.bottom;
            c->w = aw;
            c->h = ah;

            float ax = cx + (c->style.left > -1e8f ? c->style.left : 0.0f);
            if (c->style.left <= -1e8f && c->style.right > -1e8f)
                ax = cx + cw - aw - c->style.right;
            float ay = cy + (c->style.top > -1e8f ? c->style.top : 0.0f);
            if (c->style.top <= -1e8f && c->style.bottom > -1e8f)
                ay = cy + ch - ah - c->style.bottom;

            c->x = ax + c->style.margin[3];
            c->y = ay + c->style.margin[0];
            c->layout(ax, ay, aw, ah, r);
        }

        // Auto-height: expand to contain children if height not explicitly set
        if (style.explicitHeight < 0.0f) {
            float autoH = maxBottom - y + pb;
            if (autoH < 0) autoH = 0;
            if (autoH > h) h = autoH;
        }

        // Auto-width in row mode
        if (style.explicitWidth < 0.0f && isRow && maxRight > cx + cw) {
            float autoW = maxRight - x + pr;
            if (autoW > w) w = autoW;
        }

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

    virtual void draw(Renderer& r) = 0;
    virtual bool onEvent(MorphEvent& e) { return false; }
    virtual void onHover(bool state) {}

    // Content-based width (for flex / non-stretch children). Returns -1 if unknown.
    virtual float contentWidth(Renderer* r) {
        if (style.explicitWidth >= 0.0f) return style.explicitWidth;

        bool isRow = (style.display == "flex" && style.flexDirection == "row");

        if (isRow) {
            // For rows, total width = sum of children widths + gaps + margins
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

        // For columns, content width = max of children content widths
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

    // Returns the deepest node containing (ex, ey), or nullptr
    MorphNode* hitTest(float ex, float ey) {
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

    void addChild(MorphNode* child) { children.push_back(child); child->parent = this; }

    // Dispatch event to deepest child containing (ex, ey); bubble up
    // Returns true if event was handled (stops bubble)
    bool dispatchEvent(MorphEvent& e, float ex, float ey) {
        bool inBounds = (ex >= x && ex <= x + w && ey >= y && ey <= y + h);

#ifdef MORPH_FEATURE_SCROLL
        // Handle scroll wheel (only within bounds)
        if (scrollEnabled && e.type == EventType::Scroll) {
            if (inBounds) {
                scrollY -= e.scroll * 40.0f;
                if (scrollY < 0) scrollY = 0;
                if (scrollY > contentH - h) scrollY = contentH - h;
                return true;
            }
            // Not in bounds — don't consume, let children handle
        }

        // Handle scrollbar (only within bounds)
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
                // Viewport culling: skip children scrolled completely out of view
                if (scrollEnabled && (cy + c->h <= y || cy >= y + h))
                    continue;
#endif
                if (c->dispatchEvent(e, ex, ey + (scrollEnabled ? scrollY : 0)))
                    return true;
            }
        }
        return onEvent(e);
    }

    virtual ~MorphNode() {}
};
