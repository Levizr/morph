#pragma once
#include <cstdio>
#include <cmath>
#include <algorithm>
#include <string>
#include <chrono>
#include <vector>
#include <unordered_map>
#include "../core/node.h"
#include "../render/gl_renderer.h"
#include "dev_log.h"

struct DevTools {
    bool open = false;
    bool inspecting = false;
    MorphNode* hoveredNode = nullptr;
    MorphNode* selectedNode = nullptr;
    float mouseX = 0.0f, mouseY = 0.0f;
    int m_activeTab = 0; // 0 = Elements, 1 = Rendering, 2 = Logs
    DirtyStats m_lastStats;
    int m_frameCount = 0;

    static constexpr float kPanelW = 300.0f;

    // ── Frame timing (FPS) ──
    std::chrono::steady_clock::time_point m_lastFrameNow;
    float m_lastDt = 0.0f;
    float m_smoothedMs = 0.0f;
    float m_smoothedFps = 0.0f;

    // ── Repaint highlighting ──
    bool m_highlightRepaints = false;
    std::unordered_map<MorphNode*, float> m_repaintTimers;

    // ── Logs tab scrolling ──
    float m_logScroll = 0.0f;
    float m_logContentH = 0.0f;
    float m_logViewH = 0.0f;
    bool m_logDragging = false;
    float m_logDragGrabY = 0.0f;

    // ── Toast ──
    std::string m_toastText;
    float m_toastTimer = 0.0f;
    int m_toastLevel = LOG_INFO;

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

    void selectHovered() {
        if (hoveredNode) {
            selectedNode = hoveredNode;
            inspecting = false;
        }
    }

    void cancelInspect() {
        inspecting = false;
        hoveredNode = nullptr;
        selectedNode = nullptr;
    }

    void clearSelection() { selectedNode = nullptr; }

    void clearRepaintTimers() { m_repaintTimers.clear(); }

    void noteRepaint(MorphNode* n) {
        if (n) m_repaintTimers[n] = 0.35f;
    }

    void showToast(int level, const std::string& msg) {
        m_toastLevel = level;
        m_toastText = msg.size() > 96 ? msg.substr(0, 96) + "..." : msg;
        m_toastTimer = 6.0f;
    }

    void addLog(int level, const std::string& msg) {
        devLogAdd(level, msg);
        if (level == LOG_ERROR || level == LOG_WARN)
            showToast(level, msg);
    }

    void scroll(float dy) {
        if (!open || m_activeTab != 2) return;
        m_logScroll -= dy * 36.0f;
        float maxScroll = std::max(0.0f, m_logContentH - m_logViewH);
        if (m_logScroll < 0.0f) m_logScroll = 0.0f;
        if (m_logScroll > maxScroll) m_logScroll = maxScroll;
    }

    void beginLogDrag(float mx, float my, float winW, float winH) {
        if (!open || m_activeTab != 2 || m_logContentH <= m_logViewH) return;
        float px = winW - kPanelW;
        float trackX = px + kPanelW - 10.0f;
        float top = logViewTop();
        float viewH = winH - 8.0f - top;
        float thumbH = std::max(24.0f, (viewH / m_logContentH) * viewH);
        float maxScroll = m_logContentH - viewH;
        float thumbY = top + (m_logScroll / maxScroll) * (viewH - thumbH);
        if (mx < trackX || mx > trackX + 6.0f || my < top || my > top + viewH) return;
        if (my >= thumbY && my <= thumbY + thumbH)
            m_logDragGrabY = my - thumbY;
        else
            m_logDragGrabY = thumbH * 0.5f;
        m_logDragging = true;
        dragLogScroll(my, winH);
    }

    void dragLogScroll(float my, float winH) {
        if (!m_logDragging) return;
        float top = logViewTop();
        float viewH = winH - 8.0f - top;
        float thumbH = std::max(24.0f, (viewH / m_logContentH) * viewH);
        float maxScroll = m_logContentH - viewH;
        float thumbY = my - m_logDragGrabY;
        if (thumbY < top) thumbY = top;
        if (thumbY > top + viewH - thumbH) thumbY = top + viewH - thumbH;
        m_logScroll = (thumbY - top) / (viewH - thumbH) * maxScroll;
        if (m_logScroll < 0.0f) m_logScroll = 0.0f;
        if (m_logScroll > maxScroll) m_logScroll = maxScroll;
    }

    void endLogDrag() { m_logDragging = false; }

    void handleCursorPos(float mx, float my, float winW, float winH) {
        if (m_logDragging) dragLogScroll(my, winH);
    }

    void updateHover(MorphNode* root) {
        if (!inspecting || !root) {
            hoveredNode = nullptr;
            return;
        }
        hoveredNode = root->hitTest(mouseX, mouseY);
    }

    bool handleClick(float mx, float my, float winW, float winH) {
        if (!open) return false;
        float pw = kPanelW, px = winW - pw;
        float tabY = 40.0f, tabH = 28.0f, tabW = pw / 3.0f;
        // Tab clicks
        if (my >= tabY && my <= tabY + tabH) {
            if (mx >= px && mx <= px + tabW) { m_activeTab = 0; return true; }
            if (mx >= px + tabW && mx <= px + tabW * 2) { m_activeTab = 1; return true; }
            if (mx >= px + tabW * 2 && mx <= px + pw) { m_activeTab = 2; return true; }
            return false;
        }

        float contentY = tabY + tabH + 6.0f;

        if (m_activeTab == 0) {
            // Inspect button (top of Elements tab)
            float bx = px + 10.0f, by = contentY, bw = pw - 20.0f, bh = 30.0f;
            if (mx >= bx && mx <= bx + bw && my >= by && my <= by + bh) {
                toggleInspect();
                return true;
            }
            // Clear selection button (next to selected badge)
            if (selectedNode) {
                float cx = px + pw - 40.0f, cy = contentY + 54.0f, cw = 26.0f, ch = 22.0f;
                if (mx >= cx && mx <= cx + cw && my >= cy && my <= cy + ch) {
                    clearSelection();
                    return true;
                }
            }
        } else if (m_activeTab == 1) {
            // Highlight repaints toggle (bottom of Rendering tab)
            float bx = px + 10.0f, by = winH - 50.0f, bw = pw - 20.0f, bh = 30.0f;
            if (mx >= bx && mx <= bx + bw && my >= by && my <= by + bh) {
                m_highlightRepaints = !m_highlightRepaints;
                if (!m_highlightRepaints) m_repaintTimers.clear();
                return true;
            }
        } else if (m_activeTab == 2) {
            // Clear logs button (top-right of Logs tab)
            float cbx = px + pw - 76.0f, cw = 64.0f, ch = 22.0f;
            if (mx >= cbx && mx <= cbx + cw && my >= contentY && my <= contentY + ch) {
                devLogClear();
                m_logScroll = 0.0f;
                return true;
            }
            // Scrollbar drag
            beginLogDrag(mx, my, winW, winH);
            if (m_logDragging) return true;
        }
        return false;
    }

    void render(GLRenderer& r, float winW, float winH, DirtyStats& ds) {
        m_frameCount++;
        m_lastStats = ds;

        // ── Frame timing ──
        auto now = std::chrono::steady_clock::now();
        if (m_lastFrameNow.time_since_epoch().count() != 0) {
            double t = std::chrono::duration<double>(now - m_lastFrameNow).count();
            if (t > 0.25) t = 0.25;
            m_lastDt = (float)t;
            m_smoothedMs = m_smoothedMs * 0.92f + (float)(t * 1000.0) * 0.08f;
            m_smoothedFps = m_smoothedMs > 0.01f ? 1000.0f / m_smoothedMs : 0.0f;
        }
        m_lastFrameNow = now;

        // ── Decay timers ──
        if (m_lastDt > 0.0f) {
            for (auto it = m_repaintTimers.begin(); it != m_repaintTimers.end();) {
                it->second -= m_lastDt;
                if (it->second <= 0.0f) it = m_repaintTimers.erase(it);
                else ++it;
            }
            if (m_toastTimer > 0.0f) m_toastTimer -= m_lastDt;
        }

        // ── Repaint flash overlays ──
        if (m_highlightRepaints) {
            for (auto& [n, t] : m_repaintTimers) {
                if (n && t > 0.0f) drawFlash(r, n, t);
            }
        }

        // ── Toast (shown even while the panel is closed) ──
        drawToast(r, winW);

        if (!open) return;

        if (inspecting && hoveredNode)
            drawOverlay(r, hoveredNode);
        if (selectedNode)
            drawOverlay(r, selectedNode);

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

    void drawFlash(GLRenderer& r, MorphNode* n, float t) {
        float a = std::min(1.0f, t * 4.0f) * 0.28f;
        float col[4] = {0.15f, 1.0f, 0.35f, a};
        r.drawRect(n->x, screenY(n), n->w, n->h, col);
    }

    void drawToast(GLRenderer& r, float winW) {
        if (m_toastTimer <= 0.0f || m_toastText.empty()) return;
        float w = std::min(winW - 40.0f, 640.0f);
        float x = (winW - w) * 0.5f;
        float y = 12.0f, h = 42.0f;
        float bg[4] = {0.10f, 0.10f, 0.12f, 0.96f};
        r.drawRoundedRect(x, y, w, h, 6, bg);

        float edge[4] = {0.4f, 0.4f, 0.45f, 1.0f};
        switch (m_toastLevel) {
            case LOG_ERROR: edge[0] = 0.95f; edge[1] = 0.30f; edge[2] = 0.25f; break;
            case LOG_WARN:  edge[0] = 0.95f; edge[1] = 0.70f; edge[2] = 0.20f; break;
            case LOG_OK:    edge[0] = 0.20f; edge[1] = 0.80f; edge[2] = 0.30f; break;
            default: break;
        }
        r.drawRect(x, y, 4, h, edge);

        float tc[4] = {0.85f, 0.85f, 0.90f, 1.0f};
        drawTextAt(r, m_toastText, x + 14, y + 12.0f, tc, 12.0f, "normal");
        float hint[4] = {0.42f, 0.42f, 0.50f, 1.0f};
        drawTextAt(r, "Press F12 for details", x + 14, y + 27.0f, hint, 10.0f, "normal");
    }

    void drawOverlay(GLRenderer& r, MorphNode* n) {
        if (!n) return;

        float ml = n->m_computedMargin[3], mr = n->m_computedMargin[1];
        float mt = n->m_computedMargin[0], mb = n->m_computedMargin[2];
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
        float pw = kPanelW, px = winW - pw;

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
        float tabW = pw / 3.0f;
        float tabActiveBg[4] = {0.15f, 0.15f, 0.18f, 1.0f};
        float tabInactiveBg[4] = {0.10f, 0.10f, 0.12f, 1.0f};
        float tabActiveCol[4] = {0.9f, 0.9f, 0.95f, 1.0f};
        float tabInactiveCol[4] = {0.5f, 0.5f, 0.55f, 1.0f};
        const char* labels[3] = {"Elements", "Rendering", "Logs"};

        for (int i = 0; i < 3; i++) {
            float tx = px + tabW * i;
            r.drawRect(tx, tabY, tabW, tabH,
                       m_activeTab == i ? tabActiveBg : tabInactiveBg);
            float tw = r.measureTextWidth(labels[i], 11.0f, "bold");
            drawTextAt(r, labels[i], tx + (tabW - tw) * 0.5f, tabY + 6.0f,
                       m_activeTab == i ? tabActiveCol : tabInactiveCol,
                       11.0f, "bold");
        }

        float contentY = tabY + tabH + 6.0f;
        if (m_activeTab == 0)
            drawElementsTab(r, px, contentY, pw);
        else if (m_activeTab == 1)
            drawRenderingTab(r, px, contentY, pw, winH);
        else
            drawLogsTab(r, px, contentY, pw, winH);
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
        drawTextAt(r, inspecting ? "Inspecting... (F2 / Esc)" : "Inspect Element (F2)",
                   px + 16, btnY + 7.0f, btnText, 12.0f, "normal");

        if (selectedNode) {
            drawNodeInfo(r, px, btnY + 34, pw, selectedNode);
        } else if (hoveredNode) {
            drawNodeInfo(r, px, btnY + 34, pw, hoveredNode);
        } else {
            float hintCol[4] = {0.4f, 0.4f, 0.5f, 1.0f};
            drawTextAt(r, "Hover over an element", px + 14, btnY + 48, hintCol, 11.0f, "normal");
            drawTextAt(r, "to inspect", px + 14, btnY + 64, hintCol, 11.0f, "normal");
            drawTextAt(r, "Click to lock selection", px + 14, btnY + 80, hintCol, 11.0f, "normal");
        }
    }

    void drawRenderingTab(GLRenderer& r, float px, float y0, float pw, float winH) {
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
        drawTextAt(r, "FPS", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.0f", m_smoothedFps);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;
        drawTextAt(r, "Frame time", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.2f ms", m_smoothedMs);
        drawTextAt(r, buf, valX, y, m_smoothedMs > 0.0f ? colVal : colGreen, 11.0f, "normal");
        y += 18;
        drawTextAt(r, "Frame #", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%d", m_frameCount);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 18;
        drawTextAt(r, "Panel width", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.0fpx", kPanelW);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 22;

        // ── Tree stats ──
        drawSectionHeader(r, px, y, pw, "TREE");
        y += 20;
        drawTextAt(r, "Total nodes", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%d", ds.fullTreeCount);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 22;

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

        // ── Highlight repaints toggle ──
        float by = winH - 50.0f;
        float btnBg[4];
        if (m_highlightRepaints) {
            btnBg[0] = 0.15f; btnBg[1] = 0.35f; btnBg[2] = 0.6f; btnBg[3] = 1.0f;
        } else {
            btnBg[0] = 0.18f; btnBg[1] = 0.18f; btnBg[2] = 0.22f; btnBg[3] = 1.0f;
        }
        r.drawRoundedRect(px + 10, by, pw - 20, 30, 4, btnBg);
        float btnText[4] = {0.9f, 0.9f, 0.95f, 1.0f};
        drawTextAt(r, m_highlightRepaints ? "Highlight repaints: ON" : "Highlight repaints",
                   px + 16, by + 7.0f, btnText, 12.0f, "normal");
    }

    static float logViewTop() {
        return 40.0f + 28.0f + 6.0f + 30.0f;
    }

    static std::string logTimestamp(double t) {
        char buf[32];
        if (t >= 60.0) snprintf(buf, sizeof(buf), "%02dm%02.0fs", (int)(t / 60.0), (double)((int)t % 60));
        else snprintf(buf, sizeof(buf), "%5.1fs", t);
        return buf;
    }

    static std::vector<std::string> wrapLogText(GLRenderer& r, const std::string& text,
                                                float maxWidth, float fontSize) {
        std::vector<std::string> lines;
        if (text.empty()) {
            lines.push_back("");
            return lines;
        }
        std::string cur;
        float curW = 0.0f;
        size_t i = 0;
        while (i < text.size()) {
            size_t j = text.find(' ', i);
            if (j == std::string::npos) j = text.size();
            std::string word = text.substr(i, j - i);
            float wordW = r.measureTextWidth(word, fontSize, "normal");

            if (wordW > maxWidth && cur.empty()) {
                for (char c : word) {
                    float cw = r.measureTextWidth(std::string(1, c), fontSize, "normal");
                    if (curW + cw > maxWidth && !cur.empty()) {
                        lines.push_back(cur);
                        cur.clear();
                        curW = 0.0f;
                    }
                    cur += c;
                    curW += cw;
                }
            } else {
                float spaceW = cur.empty() ? 0.0f : r.measureTextWidth(" ", fontSize, "normal");
                if (!cur.empty() && curW + spaceW + wordW > maxWidth) {
                    lines.push_back(cur);
                    cur = word;
                    curW = wordW;
                } else {
                    if (!cur.empty()) { cur += ' '; curW += spaceW; }
                    cur += word;
                    curW += wordW;
                }
            }
            i = j + 1;
        }
        if (!cur.empty()) lines.push_back(cur);
        if (lines.empty()) lines.push_back("");
        return lines;
    }

    void drawLogsTab(GLRenderer& r, float px, float y0, float pw, float winH) {
        float colLbl[4] = {0.5f, 0.5f, 0.6f, 1.0f};
        float colBtn[4] = {0.9f, 0.9f, 0.95f, 1.0f};

        // ── Toolbar: title + Clear button ──
        drawSectionHeader(r, px, y0, pw, "MESSAGES");
        float clearBg[4] = {0.22f, 0.22f, 0.26f, 1.0f};
        r.drawRoundedRect(px + pw - 76.0f, y0, 64.0f, 22.0f, 4, clearBg);
        drawTextAt(r, "Clear", px + pw - 66.0f, y0 + 4.0f, colBtn, 11.0f, "normal");

        float top = y0 + 30.0f;
        float bottom = winH - 8.0f;
        float viewH = bottom - top;
        auto& entries = devLogEntries();

        float textX = px + 58.0f;
        float textMaxW = px + pw - 16.0f - textX;
        std::vector<std::vector<std::string>> wrapped;
        wrapped.reserve(entries.size());
        float contentH = 4.0f;
        for (auto& e : entries) {
            auto lines = wrapLogText(r, e.msg, textMaxW, 10.0f);
            contentH += lines.size() * 16.0f;
            wrapped.push_back(std::move(lines));
        }
        m_logContentH = contentH;
        m_logViewH = viewH;
        if (contentH > viewH && m_logScroll > contentH - viewH)
            m_logScroll = contentH - viewH;
        if (m_logScroll < 0.0f) m_logScroll = 0.0f;

        // ── Scrollable content ──
        r.beginClip(px, top, pw, viewH);

        float colInfo[4]  = {0.52f, 0.52f, 0.62f, 1.0f};
        float colOk[4]    = {0.20f, 0.80f, 0.30f, 1.0f};
        float colWarn[4]  = {0.95f, 0.70f, 0.20f, 1.0f};
        float colErr[4]   = {0.95f, 0.30f, 0.25f, 1.0f};
        float colTime[4]  = {0.38f, 0.38f, 0.46f, 1.0f};

        float lineY = top - m_logScroll + 2.0f;
        for (size_t k = 0; k < entries.size(); k++) {
            auto& e = entries[k];
            auto& lines = wrapped[k];
            if (lineY > bottom) break;
            if (lineY + lines.size() * 16.0f < top) { lineY += lines.size() * 16.0f; continue; }

            float* col = colInfo;
            switch (e.level) {
                case LOG_OK:    col = colOk; break;
                case LOG_WARN:  col = colWarn; break;
                case LOG_ERROR: col = colErr; break;
            }
            for (size_t li = 0; li < lines.size(); li++) {
                if (li == 0)
                    drawTextAt(r, logTimestamp(e.time), px + 8, lineY, colTime, 10.0f, "normal");
                drawTextAt(r, lines[li], textX, lineY, col, 10.0f, "normal");
                lineY += 16.0f;
            }
        }

        r.endClip();

        // ── Scrollbar ──
        if (contentH > viewH) {
            float trackBg[4] = {0.16f, 0.16f, 0.19f, 1.0f};
            float thumbCol[4] = {0.35f, 0.35f, 0.42f, 1.0f};
            float trackX = px + pw - 10.0f;
            r.drawRect(trackX, top, 6.0f, viewH, trackBg);
            float thumbH = std::max(24.0f, (viewH / contentH) * viewH);
            float maxScroll = contentH - viewH;
            float thumbY = top + (m_logScroll / maxScroll) * (viewH - thumbH);
            r.drawRoundedRect(trackX, thumbY, 6.0f, thumbH, 3.0f, thumbCol);
        }
    }

    void drawNodeInfo(GLRenderer& r, float px, float y0, float pw, MorphNode* n) {
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
        y += 2;
        drawSectionHeader(r, px, y, pw, "ELEMENT");
        y += 18;

        std::string tag;
        if (n->type == "__text__") {
            tag = "text";
        } else {
            tag = n->type.empty() ? "div" : n->type;
        }
        float badgeW = tag.size() * 8.0f + 22.0f;
        r.drawRoundedRect(px + 14, y, badgeW, 22, 4, tagBg);
        drawTextAt(r, "<" + tag + ">", px + 18, y + 4.0f, colWhite, 11.0f, "bold");

        // Clear selection button
        if (n == selectedNode) {
            float xBtn = px + pw - 40.0f;
            float cbBg[4] = {0.22f, 0.22f, 0.26f, 1.0f};
            float cbCol[4] = {0.8f, 0.4f, 0.4f, 1.0f};
            r.drawRoundedRect(xBtn, y, 26.0f, 22.0f, 4, cbBg);
            drawTextAt(r, "x", xBtn + 9.0f, y + 4.0f, cbCol, 12.0f, "bold");
        }

        // Breadcrumb — parent chain
        std::string trail;
        for (auto* p = n; p; p = p->parent) {
            std::string t = p->type.empty() ? "div" : p->type;
            if (t == "__text__") t = "text";
            if (!trail.empty()) trail = t + " / " + trail;
            else trail = t;
            if (trail.size() > 48) break;
        }
        if (trail.size() > 60) trail = trail.substr(trail.size() - 60);
        drawTextAt(r, trail, px + 14, y + 24.0f, colVal, 9.0f, "normal");
        y += 46;

        // ── Identity ──
        if (!n->nodeId.empty()) {
            drawTextAt(r, "ID", lblX, y, colLbl, 11.0f, "normal");
            drawTextAt(r, n->nodeId, valX, y, colVal, 11.0f, "normal");
            y += 16;
        }
        if (!n->className.empty()) {
            drawTextAt(r, "Class", lblX, y, colLbl, 11.0f, "normal");
            drawTextAt(r, n->className, valX, y, colVal, 11.0f, "normal");
            y += 16;
        }

        // ── Layout section ──
        y += 4;
        drawSectionHeader(r, px, y, pw, "LAYOUT");
        y += 18;

        drawTextAt(r, "Size", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.0f \xC3\x97 %.0f", n->w, n->h);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Position", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "(%.0f, %.0f)", n->x, n->y);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Margin", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "T:%.0f R:%.0f B:%.0f L:%.0f",
                 n->m_computedMargin[0], n->m_computedMargin[1],
                 n->m_computedMargin[2], n->m_computedMargin[3]);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Padding", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "T:%.0f R:%.0f B:%.0f L:%.0f",
                 s.padding[0], s.padding[1], s.padding[2], s.padding[3]);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 20;

        // ── Display section ──
        drawSectionHeader(r, px, y, pw, "DISPLAY");
        y += 18;

        drawTextAt(r, "Display", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.display, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Overflow", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.overflow, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Box Sizing", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.boxSizing, valX, y, colVal, 11.0f, "normal");
        y += 20;

        // ── Style section ──
        drawSectionHeader(r, px, y, pw, "STYLE");
        y += 18;

        drawTextAt(r, "Color", lblX, y, colLbl, 11.0f, "normal");
        drawSwatch(r, swatchX, y + 1, s.color);
        formatColor(buf, sizeof(buf), s.color);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Background", lblX, y, colLbl, 11.0f, "normal");
        drawSwatch(r, swatchX, y + 1, s.bgColor);
        formatColor(buf, sizeof(buf), s.bgColor);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Font Size", lblX, y, colLbl, 11.0f, "normal");
        snprintf(buf, sizeof(buf), "%.0fpx", s.fontSize);
        drawTextAt(r, buf, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Weight", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.fontWeight, valX, y, colVal, 11.0f, "normal");
        y += 16;

        drawTextAt(r, "Align", lblX, y, colLbl, 11.0f, "normal");
        drawTextAt(r, s.textAlign, valX, y, colVal, 11.0f, "normal");
    }
};
