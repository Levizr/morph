#pragma once
#include <cstdio>
#include "../core/node.h"
#include "../render/gl_renderer.h"

struct DevTools {
    bool open = false;
    bool inspecting = false;
    MorphNode* hoveredNode = nullptr;
    float mouseX = 0.0f, mouseY = 0.0f;
    int m_activeTab = 0; // 0 = Elements, 1 = Rendering
    DirtyStats m_lastStats;
    int m_frameCount = 0;

    void toggle() {
        open = !open;
        if (!open) {
            inspecting = false;
            hoveredNode = nullptr;
        }
    }

    void toggleInspect() {
        inspecting = !inspecting;
        if (!inspecting) hoveredNode = nullptr;
    }

    void updateHover(MorphNode* root) {
        if (!inspecting || !root) {
            hoveredNode = nullptr;
            return;
        }
        hoveredNode = root->hitTest(mouseX, mouseY);
    }

    bool handleClick(float mx, float my, float winW) {
        if (!open) return false;
        float pw = 300.0f, px = winW - pw;
        // Tab clicks
        float tabW = pw * 0.5f;
        float tabY = 40.0f;
        if (my >= tabY && my <= tabY + 28.0f) {
            if (mx >= px && mx <= px + tabW) { m_activeTab = 0; return true; }
            if (mx >= px + tabW && mx <= px + pw) { m_activeTab = 1; return true; }
            return false;
        }
        // Inspect button click (only in Elements tab)
        if (m_activeTab == 0) {
            float bx = px + 10.0f, by = 76.0f, bw = pw - 20.0f, bh = 30.0f;
            if (mx >= bx && mx <= bx + bw && my >= by && my <= by + bh) {
                toggleInspect();
                return true;
            }
        }
        return false;
    }

    void render(GLRenderer& r, float winW, float winH, DirtyStats& ds) {
        if (!open) return;
        m_frameCount++;
        m_lastStats = ds;

        if (inspecting && hoveredNode)
            drawOverlay(r);

        drawPanel(r, winW, winH);
    }

private:
    static float screenY(MorphNode* n) {
        float sy = n->y;
        for (auto* p = n->parent; p; p = p->parent) {
            if (p->scrollEnabled) sy -= p->scrollY;
        }
        return sy;
    }

    void drawOverlay(GLRenderer& r) {
        auto* n = hoveredNode;
        if (!n) return;

        float ml = n->style.margin[3], mr = n->style.margin[1];
        float mt = n->style.margin[0], mb = n->style.margin[2];
        float pl = n->style.padding[3], pr = n->style.padding[1];
        float pt = n->style.padding[0], pb = n->style.padding[2];
        float bw = 0.0f;
#ifdef MORPH_FEATURE_BORDER
        bw = n->style.borderWidth;
#endif

        float sx = n->x, sy = screenY(n);

        float bx = sx, by = sy, bwdt = n->w, bhgt = n->h;

        float pdx = bx + bw, pdy = by + bw;
        float pdw = bwdt - bw * 2.0f, pdh = bhgt - bw * 2.0f;
        if (pdw < 0.0f) pdw = 0.0f;
        if (pdh < 0.0f) pdh = 0.0f;

        float cx = pdx + pl, cy = pdy + pt;
        float cw = pdw - pl - pr, ch = pdh - pt - pb;
        if (cw < 0.0f) cw = 0.0f;
        if (ch < 0.0f) ch = 0.0f;

        float mdx = bx - ml, mdy = by - mt;
        float mdw = bwdt + ml + mr, mdh = bhgt + mt + mb;

        float colMargin[4]  = {1.0f, 0.6f, 0.0f, 0.22f};
        float colBorder[4]  = {1.0f, 0.85f, 0.0f, 0.25f};
        float colPadding[4] = {0.0f, 0.8f, 0.2f, 0.20f};
        float colContent[4] = {0.2f, 0.4f, 1.0f, 0.18f};

        auto drawRing = [&](float ox, float oy, float ow, float oh,
                            float top, float right, float bottom, float left,
                            float col[4]) {
            if (ow < 0.01f || oh < 0.01f) return;
            if (top > 0.0f)    r.drawRect(ox, oy, ow, top, col);
            if (bottom > 0.0f) r.drawRect(ox, oy + oh - bottom, ow, bottom, col);
            float innerH = oh - top - bottom;
            if (innerH > 0.0f && left > 0.0f)
                r.drawRect(ox, oy + top, left, innerH, col);
            if (innerH > 0.0f && right > 0.0f)
                r.drawRect(ox + ow - right, oy + top, right, innerH, col);
        };

        // Margin ring (outside border)
        drawRing(mdx, mdy, mdw, mdh, mt, mr, mb, ml, colMargin);
        // Border ring (between border edge and padding edge)
        drawRing(bx, by, bwdt, bhgt, bw, bw, bw, bw, colBorder);
        // Padding ring (between padding edge and content edge)
        drawRing(pdx, pdy, pdw, pdh, pt, pr, pb, pl, colPadding);
        // Content (innermost fill)
        if (cw > 0.01f && ch > 0.01f)
            r.drawRect(cx, cy, cw, ch, colContent);
    }

    // drawText treats y as "top of text area" — baseline = y + fontSize.
    // The visible glyphs sit ~2-3px below y (due to baseline offset).
    static void drawTextAt(GLRenderer& r, const std::string& text,
                           float x, float y, float color[4],
                           float fontSize, const std::string& fontWeight) {
        r.drawText(text, x, y, color, TextAlign::Left, fontSize, fontWeight);
    }

    static void drawSectionHeader(GLRenderer& r, float px, float y, float pw,
                                  const std::string& label) {
        float accent[4] = {0.4f, 0.7f, 1.0f, 1.0f};
        r.drawRect(px + 12, y + 3, 2, 10, accent);
        float labelCol[4] = {0.45f, 0.45f, 0.55f, 1.0f};
        drawTextAt(r, label, px + 20, y, labelCol, 10.0f, "bold");
    }

    static void formatColor(char* buf, size_t n, float c[4]) {
        int r = (int)(c[0] * 255.0f + 0.5f);
        int g = (int)(c[1] * 255.0f + 0.5f);
        int b = (int)(c[2] * 255.0f + 0.5f);
        if (c[3] > 0.999f)
            snprintf(buf, n, "#%02X%02X%02X", r, g, b);
        else
            snprintf(buf, n, "rgba(%d,%d,%d,%.1g)", r, g, b, c[3]);
    }

    static void drawSwatch(GLRenderer& r, float x, float y, float color[4]) {
        float border[4] = {0.3f, 0.3f, 0.35f, 1.0f};
        r.drawRect(x, y, 10, 10, border);
        r.drawRect(x + 1, y + 1, 8, 8, color);
    }

    void drawPanel(GLRenderer& r, float winW, float winH) {
        float pw = 300.0f, px = winW - pw;

        float panelBg[4] = {0.09f, 0.09f, 0.11f, 0.93f};
        r.drawRect(px, 0, pw, winH, panelBg);

        float divider[4] = {0.2f, 0.2f, 0.22f, 1.0f};
        r.drawRect(px, 0, 1, winH, divider);

        float headerBg[4] = {0.12f, 0.12f, 0.14f, 1.0f};
        r.drawRect(px, 0, pw, 38, headerBg);
        float headerCol[4] = {0.8f, 0.8f, 0.85f, 1.0f};
        drawTextAt(r, "DevTools", px + 14, 10.0f, headerCol, 14.0f, "bold");

        float keyHint[4] = {0.35f, 0.35f, 0.42f, 1.0f};
        drawTextAt(r, "F12", px + pw - 40, 12.0f, keyHint, 11.0f, "normal");

        // ── Tabs ──
        float tabY = 40.0f;
        float tabH = 28.0f;
        float tabW = pw * 0.5f;
        float tabActiveBg[4] = {0.15f, 0.15f, 0.18f, 1.0f};
        float tabInactiveBg[4] = {0.10f, 0.10f, 0.12f, 1.0f};
        float tabActiveCol[4] = {0.9f, 0.9f, 0.95f, 1.0f};
        float tabInactiveCol[4] = {0.5f, 0.5f, 0.55f, 1.0f};

        r.drawRect(px, tabY, tabW, tabH, m_activeTab == 0 ? tabActiveBg : tabInactiveBg);
        drawTextAt(r, "Elements", px + tabW * 0.5f - 28, tabY + 6.0f,
                   m_activeTab == 0 ? tabActiveCol : tabInactiveCol, 11.0f, "bold");
        r.drawRect(px + tabW, tabY, tabW, tabH, m_activeTab == 1 ? tabActiveBg : tabInactiveBg);
        drawTextAt(r, "Rendering", px + tabW + tabW * 0.5f - 30, tabY + 6.0f,
                   m_activeTab == 1 ? tabActiveCol : tabInactiveCol, 11.0f, "bold");

        float contentY = tabY + tabH + 6.0f;
        if (m_activeTab == 0)
            drawElementsTab(r, px, contentY, pw);
        else
            drawRenderingTab(r, px, contentY, pw);
    }

    void drawElementsTab(GLRenderer& r, float px, float y0, float pw) {
        float btnY = y0;
        float btnBg[4];
        if (inspecting) {
            btnBg[0] = 0.15f; btnBg[1] = 0.35f; btnBg[2] = 0.6f; btnBg[3] = 1.0f;
        } else {
            btnBg[0] = 0.18f; btnBg[1] = 0.18f; btnBg[2] = 0.22f; btnBg[3] = 1.0f;
        }
        r.drawRoundedRect(px + 10, btnY, pw - 20, 30, 4, btnBg);

        float btnText[4] = {0.9f, 0.9f, 0.95f, 1.0f};
        drawTextAt(r, inspecting ? "Inspecting... (F2)" : "Inspect Element (F2)",
                   px + 16, btnY + 7.0f, btnText, 12.0f, "normal");

        if (hoveredNode) {
            drawNodeInfo(r, px, btnY + 50, pw);
        } else {
            float hintCol[4] = {0.4f, 0.4f, 0.5f, 1.0f};
            drawTextAt(r, "Hover over an element", px + 14, btnY + 64, hintCol, 11.0f, "normal");
            drawTextAt(r, "to inspect", px + 14, btnY + 80, hintCol, 11.0f, "normal");
        }
    }

    void drawRenderingTab(GLRenderer& r, float px, float y0, float pw) {
        float colLbl[4] = {0.5f, 0.5f, 0.6f, 1.0f};
        float colVal[4] = {0.75f, 0.75f, 0.85f, 1.0f};
        float colGreen[4] = {0.2f, 0.8f, 0.3f, 1.0f};
        float colRed[4] = {0.9f, 0.3f, 0.2f, 1.0f};
        float y = y0;
        char buf[128];
        float lblX = px + 14;
        float valX = px + 120;

        auto& ds = m_lastStats;

        // ── Frame info ──
        drawSectionHeader(r, px, y, pw, "FRAME");
        y += 20;
        drawTextAt(r, "Frame #", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%d", m_frameCount);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;

        // ── Tree stats ──
        drawSectionHeader(r, px, y, pw, "TREE");
        y += 20;
        drawTextAt(r, "Total nodes", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%d", ds.fullTreeCount);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;

        // ── Layout stats ──
        drawSectionHeader(r, px, y, pw, "LAYOUT");
        y += 20;
        drawTextAt(r, "Laid out", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%d", ds.layoutCount);
        drawTextAt(r, buf, valX, y, ds.layoutCount > 0 ? colRed : colGreen, 11.0f, "normal");
        y += 18;
        drawTextAt(r, "Skipped", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%d", ds.skippedCount);
        drawTextAt(r, buf, valX, y, colGreen, 11.0f, "normal");
        y += 18;
        float pct = ds.fullTreeCount > 0 ? (ds.layoutCount * 100.0f / ds.fullTreeCount) : 0;
        drawTextAt(r, "Layout %", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.1f%%", pct);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 22;

        // ── Paint stats ──
        drawSectionHeader(r, px, y, pw, "PAINT");
        y += 20;
        drawTextAt(r, "Repainted", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%d", ds.paintCount);
        drawTextAt(r, buf, valX, y, ds.paintCount > 0 ? colRed : colGreen, 11.0f, "normal");
        y += 18;
        float saved = ds.fullTreeCount - ds.paintCount;
        drawTextAt(r, "Cache hit", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%d (%.0f%%)", (int)(saved > 0 ? saved : 0),
                 ds.fullTreeCount > 0 ? (saved * 100.0f / ds.fullTreeCount) : 0);
        drawTextAt(r, buf, valX, y, colGreen, 11.0f, "normal");
        y += 22;

        // ── Savings ──
        drawSectionHeader(r, px, y, pw, "SAVINGS");
        y += 20;
        int savedLayout = ds.fullTreeCount - ds.layoutCount;
        int savedPaint = ds.fullTreeCount - ds.paintCount;
        float layoutSavings = ds.fullTreeCount > 0 ? (savedLayout * 100.0f / ds.fullTreeCount) : 0;
        float paintSavings = ds.fullTreeCount > 0 ? (savedPaint * 100.0f / ds.fullTreeCount) : 0;
        drawTextAt(r, "Layout saved", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.0f%%", layoutSavings);
        drawTextAt(r, buf, valX, y, layoutSavings > 50 ? colGreen : colRed, 11.0f, "normal");
        y += 18;
        drawTextAt(r, "Paint saved", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.0f%%", paintSavings);
        drawTextAt(r, buf, valX, y, paintSavings > 50 ? colGreen : colRed, 11.0f, "normal");
    }

    void drawNodeInfo(GLRenderer& r, float px, float y0, float pw) {
        auto* n = hoveredNode;
        if (!n) return;
        auto& s = n->style;

        float colLbl[4] = {0.5f, 0.5f, 0.6f, 1.0f};
        float colVal[4] = {0.75f, 0.75f, 0.85f, 1.0f};
        float colWhite[4] = {0.95f, 0.95f, 0.98f, 1.0f};
        float tagBg[4] = {0.15f, 0.35f, 0.6f, 1.0f};
        float y = y0;
        char buf[128];

        float lblX = px + 14;
        float valX = px + 85;
        float swatchX = px + pw - 28;

        // ── Element badge ──
        y += 6;
        drawSectionHeader(r, px, y, pw, "ELEMENT");
        y += 20;

        std::string tag;
        if (n->type == "__text__") {
            tag = "text";
        } else {
            tag = n->type.empty() ? "div" : n->type;
        }
        float badgeW = tag.size() * 8.0f + 22.0f;
        r.drawRoundedRect(px + 14, y, badgeW, 22, 4, tagBg);
        drawTextAt(r, "<" + tag + ">", px + 18, y + 4.0f, colWhite, 11.0f, "bold");
        y += 32;

        // ── Layout section ──
        drawSectionHeader(r, px, y, pw, "LAYOUT");
        y += 20;

        drawTextAt(r, "Size", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.0f \xC3\x97 %.0f", n->w, n->h);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Position", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "(%.0f, %.0f)", n->x, n->y);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Margin", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "T:%.0f R:%.0f B:%.0f L:%.0f",
                 s.margin[0], s.margin[1], s.margin[2], s.margin[3]);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Padding", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "T:%.0f R:%.0f B:%.0f L:%.0f",
                 s.padding[0], s.padding[1], s.padding[2], s.padding[3]);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 22;

        // ── Display section ──
        drawSectionHeader(r, px, y, pw, "DISPLAY");
        y += 20;

        drawTextAt(r, "Display", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.display, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Overflow", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.overflow, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Box Sizing", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.boxSizing, valX, y, colVal, 11.0f, "normal");
        y += 22;

        // ── Style section ──
        drawSectionHeader(r, px, y, pw, "STYLE");
        y += 20;

        drawTextAt(r, "Color", lblX, y, colLbl, 11.0f, "normal");
        drawSwatch(r, swatchX, y + 1, s.color);
        formatColor(buf, sizeof(buf), s.color);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Background", lblX, y, colLbl, 11.0f, "normal");
        drawSwatch(r, swatchX, y + 1, s.bgColor);
        formatColor(buf, sizeof(buf), s.bgColor);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Font Size", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.0fpx", s.fontSize);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Weight", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.fontWeight, valX, y, colVal, 11.0f, "normal");
        y += 18;

        drawTextAt(r, "Align", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.textAlign, valX, y, colVal, 11.0f, "normal");
    }
};
