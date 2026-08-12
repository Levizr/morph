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
#include "../renderers/renderer.h"
#include "dev_log.h"
#include "dev_net.h"

struct DevTools {
    bool open = false;
    bool inspecting = false;
    MorphNode* hoveredNode = nullptr;
    MorphNode* selectedNode = nullptr;
    float mouseX = 0.0f, mouseY = 0.0f;
    int m_activeTab = 0; // 0 = Elements, 1 = Rendering, 2 = Network, 3 = Logs
    DirtyStats m_lastStats;
    int m_frameCount = 0;

    static constexpr float kMinPanelW = 240.0f;
    float m_panelW = 320.0f;

    // ── Panel resize drag ──
    bool m_resizing = false;
    float m_resizeGrabX = 0.0f;

    // ── Frame timing (FPS) ──
    std::chrono::steady_clock::time_point m_lastFrameNow;
    float m_lastDt = 0.0f;
    float m_smoothedMs = 0.0f;
    float m_smoothedFps = 0.0f;

    // ── Repaint highlighting ──
    bool m_highlightRepaints = false;
    std::unordered_map<MorphNode*, float> m_repaintTimers;

    // ── Scroll state (Network + Logs tabs) ──
    struct ScrollState {
        float scroll = 0.0f;
        float contentH = 0.0f;
        float viewH = 0.0f;
        bool dragging = false;
        float dragGrabY = 0.0f;
    };
    ScrollState m_logScroll;
    ScrollState m_netScroll;

    // ── Network detail view ──
    int m_selectedNetId = 0;   // id of the entry shown in the detail view (0 = list)

    // ── Toast ──
    std::string m_toastText;
    float m_toastTimer = 0.0f;
    int m_toastLevel = LOG_INFO;

    // ── Layout geometry (shared by draw + click handling) ──
    static constexpr float kHeaderH = 54.0f;
    static constexpr float kTabY = 56.0f;
    static constexpr float kTabH = 30.0f;
    static constexpr float kContentY = 92.0f;

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
        if (!open) return;
        if (m_activeTab != 2 && m_activeTab != 3) return;
        ScrollState& ss = (m_activeTab == 3) ? m_logScroll : m_netScroll;
        ss.scroll -= dy * 36.0f;
        float maxScroll = std::max(0.0f, ss.contentH - ss.viewH);
        if (ss.scroll < 0.0f) ss.scroll = 0.0f;
        if (ss.scroll > maxScroll) ss.scroll = maxScroll;
    }

    void beginScrollDrag(float mx, float my, float winW, float winH, ScrollState& ss) {
        if (!open || ss.contentH <= ss.viewH) return;
        float px = winW - m_panelW;
        float trackX = px + m_panelW - 10.0f;
        float top = logViewTop();
        float viewH = winH - 8.0f - top;
        float thumbH = std::max(24.0f, (viewH / ss.contentH) * viewH);
        float maxScroll = ss.contentH - viewH;
        float thumbY = top + (ss.scroll / maxScroll) * (viewH - thumbH);
        if (mx < trackX || mx > trackX + 6.0f || my < top || my > top + viewH) return;
        if (my >= thumbY && my <= thumbY + thumbH)
            ss.dragGrabY = my - thumbY;
        else
            ss.dragGrabY = thumbH * 0.5f;
        ss.dragging = true;
        dragScroll(my, winH, ss);
    }

    void dragScroll(float my, float winH, ScrollState& ss) {
        if (!ss.dragging) return;
        float top = logViewTop();
        float viewH = winH - 8.0f - top;
        float thumbH = std::max(24.0f, (viewH / ss.contentH) * viewH);
        float maxScroll = ss.contentH - viewH;
        float thumbY = my - ss.dragGrabY;
        if (thumbY < top) thumbY = top;
        if (thumbY > top + viewH - thumbH) thumbY = top + viewH - thumbH;
        ss.scroll = (thumbY - top) / (viewH - thumbH) * maxScroll;
        if (ss.scroll < 0.0f) ss.scroll = 0.0f;
        if (ss.scroll > maxScroll) ss.scroll = maxScroll;
    }

    void endLogDrag() {
        m_logScroll.dragging = false;
        m_netScroll.dragging = false;
    }

    void handleCursorPos(float mx, float my, float winW, float winH) {
        if (m_activeTab == 2 && m_netScroll.dragging)
            dragScroll(my, winH, m_netScroll);
        else if (m_activeTab == 3 && m_logScroll.dragging)
            dragScroll(my, winH, m_logScroll);
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
        float pw = m_panelW, px = winW - pw;

        // ── Segmented tab control ──
        if (my >= kTabY && my <= kTabY + kTabH) {
            float cX = px + 10, cW = pw - 20, segW = cW / 4.0f;
            if (mx >= cX && mx <= cX + cW) {
                int idx = (int)((mx - cX) / segW);
                if (idx < 0) idx = 0;
                if (idx > 3) idx = 3;
                m_activeTab = idx;
                return true;
            }
            return false;
        }

        if (m_activeTab == 0) {
            // Inspect button
            float bx = px + 10, by = kContentY, bw = pw - 20, bh = 34.0f;
            if (mx >= bx && mx <= bx + bw && my >= by && my <= by + bh) {
                toggleInspect();
                return true;
            }
            // Clear selection button (badge row of selected node)
            if (selectedNode) {
                float badgeY = kContentY + 42.0f + 22.0f;
                float cx = px + pw - 44.0f, cw = 24.0f, ch = 22.0f;
                if (mx >= cx && mx <= cx + cw && my >= badgeY && my <= badgeY + ch) {
                    clearSelection();
                    return true;
                }
            }
        } else if (m_activeTab == 1) {
#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
            // Flash | Forge segmented renderer switch
            float segY = kContentY + 52.0f;
            float cW = pw - 20, half = (cW - 6.0f) * 0.5f;
            if (my >= segY && my <= segY + 28.0f && mx >= px + 10 && mx <= px + 10 + cW) {
                RenderMode m = (mx < px + 10 + 3.0f + half) ? RenderMode::Flash : RenderMode::Forge;
                setRenderMode(m);
                return true;
            }
#endif
            // Highlight repaints toggle switch
            float ty = winH - 46.0f;
            if (mx >= px + 10 && mx <= px + pw - 10 && my >= ty && my <= ty + 30.0f) {
                m_highlightRepaints = !m_highlightRepaints;
                if (!m_highlightRepaints) m_repaintTimers.clear();
                return true;
            }
        } else if (m_activeTab == 2) {
            if (netDetailOpen()) {
                // Back button
                if (mx >= px + 10 && mx <= px + 74 && my >= kContentY && my <= kContentY + 22) {
                    m_selectedNetId = 0;
                    m_netScroll.scroll = 0.0f;
                    return true;
                }
                // Scrollbar drag
                beginScrollDrag(mx, my, winW, winH, m_netScroll);
                if (m_netScroll.dragging) return true;
            } else {
                // Clear requests button
                float cbx = px + pw - 78.0f, cw = 66.0f, ch = 22.0f;
                if (mx >= cbx && mx <= cbx + cw && my >= kContentY && my <= kContentY + ch) {
                    devNetClear();
                    m_netScroll.scroll = 0.0f;
                    return true;
                }
                // Row click → open detail view
                float top = logViewTop();
                float rowH = 24.0f;
                if (my >= top && my <= winH - 8.0f && mx >= px + 8 && mx <= px + pw - 14.0f) {
                    auto entries = devNetSnapshot();
                    int idx = (int)((my - top + m_netScroll.scroll) / rowH);
                    if (idx >= 0 && idx < (int)entries.size()) {
                        m_selectedNetId = entries[idx].id;
                        m_netScroll.scroll = 0.0f;
                        return true;
                    }
                }
                // Scrollbar drag
                beginScrollDrag(mx, my, winW, winH, m_netScroll);
                if (m_netScroll.dragging) return true;
            }
        } else if (m_activeTab == 3) {
            // Clear logs button
            float cbx = px + pw - 78.0f, cw = 66.0f, ch = 22.0f;
            if (mx >= cbx && mx <= cbx + cw && my >= kContentY && my <= kContentY + ch) {
                devLogClear();
                m_logScroll.scroll = 0.0f;
                return true;
            }
            // Scrollbar drag
            beginScrollDrag(mx, my, winW, winH, m_logScroll);
            if (m_logScroll.dragging) return true;
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
        float y = 12.0f, h = 46.0f;
        float bg[4] = {0.086f, 0.094f, 0.122f, 0.97f};
        r.drawRoundedRect(x, y, w, h, 10, bg);
        float border[4] = {0.16f, 0.18f, 0.24f, 1.0f};
        r.drawBorderRing(x, y, w, h, 10, 1.0f, border);

        float edge[4] = {0.486f, 0.416f, 0.961f, 1.0f};
        switch (m_toastLevel) {
            case LOG_ERROR: edge[0] = 0.95f; edge[1] = 0.32f; edge[2] = 0.22f; break;
            case LOG_WARN:  edge[0] = 0.95f; edge[1] = 0.70f; edge[2] = 0.20f; break;
            case LOG_OK:    edge[0] = 0.12f; edge[1] = 0.79f; edge[2] = 0.54f; break;
            default: break;
        }
        r.drawRoundedRect(x + 8, y + 9, 4, h - 18, 2, edge);

        float tc[4] = {0.90f, 0.91f, 0.95f, 1.0f};
        drawTextAt(r, m_toastText, x + 20, y + 13.0f, tc, 12.0f, "normal");
        float hint[4] = {0.50f, 0.52f, 0.62f, 1.0f};
        drawTextAt(r, "Press F12 for details", x + 20, y + 29.0f, hint, 10.0f, "normal");
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

    static void drawCard(GLRenderer& r, float x, float y, float w, float h) {
        float bg[4] = {0.055f, 0.061f, 0.082f, 0.96f};
        float border[4] = {0.125f, 0.14f, 0.19f, 1.0f};
        r.drawBorderedRoundedRect(x, y, w, h, 8.0f, bg, 1.0f, border);
    }

    static void drawSectionLabel(GLRenderer& r, float x, float y,
                                 const std::string& label) {
        float accent[4] = {0.486f, 0.416f, 0.961f, 1.0f};
        r.drawRoundedRect(x, y + 2, 3, 12, 1.5f, accent);
        float lbl[4] = {0.55f, 0.58f, 0.68f, 1.0f};
        drawTextAt(r, label, x + 9, y, lbl, 9.0f, "bold");
    }

    static void drawRow(GLRenderer& r, float x, float y, const char* label,
                        const char* val, float valCol[4]) {
        float lbl[4] = {0.55f, 0.57f, 0.66f, 1.0f};
        drawTextAt(r, label, x, y, lbl, 11.0f, "normal");
        drawTextAt(r, val, x + 90.0f, y, valCol, 11.0f, "normal");
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
        float border[4] = {0.14f, 0.16f, 0.21f, 1.0f};
        r.drawRoundedRect(x, y, 12, 12, 3, border);
        r.drawRoundedRect(x + 1, y + 1, 10, 10, 2, color);
    }

    static void drawSwitch(GLRenderer& r, float x, float y, bool on) {
        float trackW = 36.0f, trackH = 18.0f;
        if (on) {
            float onCol[4] = {0.486f, 0.416f, 0.961f, 1.0f};
            r.drawRoundedRect(x, y, trackW, trackH, 9.0f, onCol);
            float knob[4] = {1.0f, 1.0f, 1.0f, 1.0f};
            r.drawRoundedRect(x + trackW - 16.0f, y + 2.0f, 14.0f, 14.0f, 7.0f, knob);
        } else {
            float offCol[4] = {0.13f, 0.14f, 0.17f, 1.0f};
            r.drawRoundedRect(x, y, trackW, trackH, 9.0f, offCol);
            float knob[4] = {0.5f, 0.52f, 0.6f, 1.0f};
            r.drawRoundedRect(x + 2.0f, y + 2.0f, 14.0f, 14.0f, 7.0f, knob);
        }
    }

    void drawPanel(GLRenderer& r, float winW, float winH) {
        float pw = m_panelW, px = winW - pw;

        // ── Panel background (opaque — the docked panel covers the app strip) ──
        float panelBg[4] = {0.055f, 0.059f, 0.076f, 1.0f};
        r.drawRect(px, 0, pw, winH, panelBg);
        float divider[4] = {0.13f, 0.14f, 0.18f, 1.0f};
        r.drawRect(px, 0, 1, winH, divider);

        // ── Resize handle (left edge) ──
        float handleCol[4];
        float handleLine[4] = {0.486f, 0.416f, 0.961f, 1.0f};
        if (m_resizing) {
            handleCol[0] = 0.486f; handleCol[1] = 0.416f; handleCol[2] = 0.961f; handleCol[3] = 1.0f;
        } else {
            handleCol[0] = 0.19f; handleCol[1] = 0.21f; handleCol[2] = 0.28f; handleCol[3] = 1.0f;
        }
        r.drawRect(px - 3, 0, 3, winH, handleCol);
        r.drawRect(px - 3, 0, 1, winH, handleLine);

        drawHeader(r, px, pw);
        drawTabs(r, px, pw);

        if (m_activeTab == 0)
            drawElementsTab(r, px, kContentY, pw);
        else if (m_activeTab == 1)
            drawRenderingTab(r, px, kContentY, pw, winH);
        else if (m_activeTab == 2)
            drawNetworkTab(r, px, kContentY, pw, winH);
        else
            drawLogsTab(r, px, kContentY, pw, winH);
    }

    // ── Branded header: logo mark + morph wordmark ──
    void drawHeader(GLRenderer& r, float px, float pw) {
        // Accent stripe along the top
        float stripe[4] = {0.486f, 0.416f, 0.961f, 1.0f};
        r.drawRect(px, 0, pw, 3, stripe);

        float headerBg[4] = {0.086f, 0.094f, 0.122f, 1.0f};
        r.drawRect(px, 3, pw, kHeaderH - 3, headerBg);

        // Logo mark
        float logoBg[4] = {0.486f, 0.416f, 0.961f, 1.0f};
        r.drawRoundedRect(px + 12, 11, 28, 28, 8, logoBg);
        float mw = r.measureTextWidth("m", 17.0f, "bold");
        float white[4] = {1.0f, 1.0f, 1.0f, 1.0f};
        drawTextAt(r, "m", px + 12 + (28.0f - mw) * 0.5f, 15.0f, white, 17.0f, "bold");

        // Wordmark + subtitle
        float word[4] = {0.94f, 0.95f, 0.98f, 1.0f};
        drawTextAt(r, "morph", px + 50, 9.0f, word, 17.0f, "bold");
        float sub[4] = {0.49f, 0.52f, 0.62f, 1.0f};
        drawTextAt(r, "DEVELOPER TOOLS", px + 51, 31.0f, sub, 9.0f, "bold");

        // F12 key-cap chip
        float chipBg[4] = {0.12f, 0.13f, 0.17f, 1.0f};
        float chipBorder[4] = {0.18f, 0.20f, 0.26f, 1.0f};
        r.drawBorderedRoundedRect(px + pw - 58, 13, 44, 22, 6, chipBg, 1.0f, chipBorder);
        float chipCol[4] = {0.55f, 0.58f, 0.68f, 1.0f};
        drawTextAt(r, "F12", px + pw - 53, 17.0f, chipCol, 10.0f, "bold");
    }

    // ── Segmented pill tab control ──
    void drawTabs(GLRenderer& r, float px, float pw) {
        float containerW = pw - 20.0f, segW = (containerW - 6.0f) / 4.0f;
        float containerBg[4] = {0.043f, 0.047f, 0.063f, 1.0f};
        r.drawRoundedRect(px + 10, kTabY, containerW, kTabH, 8, containerBg);

        const char* labels[4] = {"Elements", "Rendering", "Network", "Logs"};
        for (int i = 0; i < 4; i++) {
            float pillX = px + 10 + 3 + segW * i;
            if (m_activeTab == i) {
                float pill[4] = {0.486f, 0.416f, 0.961f, 1.0f};
                r.drawRoundedRect(pillX, kTabY + 3, segW, kTabH - 6, 6, pill);
            }
            float tw = r.measureTextWidth(labels[i], 11.0f, "bold");
            float col[4];
            if (m_activeTab == i) {
                col[0] = 1.0f; col[1] = 1.0f; col[2] = 1.0f; col[3] = 1.0f;
            } else {
                col[0] = 0.50f; col[1] = 0.53f; col[2] = 0.64f; col[3] = 1.0f;
            }
            drawTextAt(r, labels[i], pillX + (segW - tw) * 0.5f, kTabY + 5.0f,
                       col, 11.0f, "bold");
        }
    }

    void drawElementsTab(GLRenderer& r, float px, float y0, float pw) {
        // ── Primary inspect button ──
        float btnBg[4], btnCol[4];
        if (inspecting) {
            btnBg[0] = 0.486f; btnBg[1] = 0.416f; btnBg[2] = 0.961f; btnBg[3] = 1.0f;
            btnCol[0] = 1.0f; btnCol[1] = 1.0f; btnCol[2] = 1.0f; btnCol[3] = 1.0f;
        } else {
            btnBg[0] = 0.086f; btnBg[1] = 0.094f; btnBg[2] = 0.122f; btnBg[3] = 1.0f;
            btnCol[0] = 0.85f; btnCol[1] = 0.87f; btnCol[2] = 0.92f; btnCol[3] = 1.0f;
        }
        float btnBorder[4] = {0.17f, 0.19f, 0.24f, 1.0f};
        r.drawBorderedRoundedRect(px + 10, y0, pw - 20, 34, 8, btnBg, 1.0f, btnBorder);
        drawTextAt(r, inspecting ? "Inspecting  \xC2\xB7  Esc to stop" : "Inspect Element  \xC2\xB7  F2",
                   px + 18, y0 + 9.0f, btnCol, 12.0f, "bold");

        float cy = y0 + 42.0f;
        if (selectedNode) {
            drawNodeInfo(r, px, cy, pw, selectedNode);
        } else if (hoveredNode) {
            drawNodeInfo(r, px, cy, pw, hoveredNode);
        } else {
            drawCard(r, px + 10, cy, pw - 20, 76);
            float hintCol[4] = {0.46f, 0.49f, 0.59f, 1.0f};
            drawTextAt(r, "Hover over an element", px + 22, cy + 14, hintCol, 11.0f, "normal");
            drawTextAt(r, "to inspect it live", px + 22, cy + 31, hintCol, 11.0f, "normal");
            drawTextAt(r, "Click to lock the selection", px + 22, cy + 48, hintCol, 11.0f, "normal");
        }
    }

    void drawRenderingTab(GLRenderer& r, float px, float y0, float pw, float winH) {
        auto& ds = m_lastStats;
        char buf[160];
        float cardX = px + 10, cardW = pw - 20;
        float valCol[4] = {0.83f, 0.85f, 0.91f, 1.0f};
        float green[4] = {0.12f, 0.79f, 0.54f, 1.0f};
        float red[4] = {0.95f, 0.32f, 0.22f, 1.0f};
        float y = y0;

        // ── RENDERER card (status + toggle) ──
        drawRendererCard(r, px, y, pw);
        y += 96.0f + 8.0f;

        // ── FRAME card ──
        float cardH = 34.0f + 4 * 17.0f;
        drawCard(r, cardX, y, cardW, cardH);
        drawSectionLabel(r, px + 22, y + 6, "FRAME");
        float ry = y + 26.0f;
        snprintf(buf, sizeof(buf), "%.0f", m_smoothedFps);
        drawRow(r, px + 22, ry, "FPS", buf, valCol); ry += 17.0f;
        snprintf(buf, sizeof(buf), "%.2f ms", m_smoothedMs);
        drawRow(r, px + 22, ry, "Frame time", buf, valCol); ry += 17.0f;
        snprintf(buf, sizeof(buf), "%d", m_frameCount);
        drawRow(r, px + 22, ry, "Frame #", buf, valCol); ry += 17.0f;
        snprintf(buf, sizeof(buf), "%d", ds.fullTreeCount);
        drawRow(r, px + 22, ry, "Total nodes", buf, valCol);
        y += cardH + 8.0f;

        // ── LAYOUT card ──
        cardH = 34.0f + 3 * 17.0f;
        drawCard(r, cardX, y, cardW, cardH);
        drawSectionLabel(r, px + 22, y + 6, "LAYOUT");
        ry = y + 26.0f;
        snprintf(buf, sizeof(buf), "%d", ds.layoutCount);
        drawRow(r, px + 22, ry, "Laid out", buf, ds.layoutCount > 0 ? red : green); ry += 17.0f;
        snprintf(buf, sizeof(buf), "%d", ds.skippedCount);
        drawRow(r, px + 22, ry, "Skipped", buf, green); ry += 17.0f;
        float pct = ds.fullTreeCount > 0 ? (ds.layoutCount * 100.0f / ds.fullTreeCount) : 0;
        snprintf(buf, sizeof(buf), "%.1f%%", pct);
        drawRow(r, px + 22, ry, "Layout %", buf, valCol);
        y += cardH + 8.0f;

        // ── PAINT card ──
        cardH = 34.0f + 2 * 17.0f;
        drawCard(r, cardX, y, cardW, cardH);
        drawSectionLabel(r, px + 22, y + 6, "PAINT");
        ry = y + 26.0f;
        snprintf(buf, sizeof(buf), "%d", ds.paintCount);
        drawRow(r, px + 22, ry, "Repainted", buf, ds.paintCount > 0 ? red : green); ry += 17.0f;
        float saved = ds.fullTreeCount - ds.paintCount;
        snprintf(buf, sizeof(buf), "%d (%.0f%%)", (int)(saved > 0 ? saved : 0),
                 ds.fullTreeCount > 0 ? (saved * 100.0f / ds.fullTreeCount) : 0);
        drawRow(r, px + 22, ry, "Cache hit", buf, green);
        y += cardH + 8.0f;

        // ── SAVINGS card ──
        cardH = 34.0f + 2 * 17.0f;
        drawCard(r, cardX, y, cardW, cardH);
        drawSectionLabel(r, px + 22, y + 6, "SAVINGS");
        ry = y + 26.0f;
        int savedLayout = ds.fullTreeCount - ds.layoutCount;
        int savedPaint = ds.fullTreeCount - ds.paintCount;
        float layoutSavings = ds.fullTreeCount > 0 ? (savedLayout * 100.0f / ds.fullTreeCount) : 0;
        float paintSavings = ds.fullTreeCount > 0 ? (savedPaint * 100.0f / ds.fullTreeCount) : 0;
        snprintf(buf, sizeof(buf), "%.0f%%", layoutSavings);
        drawRow(r, px + 22, ry, "Layout saved", buf, layoutSavings > 50 ? green : red); ry += 17.0f;
        snprintf(buf, sizeof(buf), "%.0f%%", paintSavings);
        drawRow(r, px + 22, ry, "Paint saved", buf, paintSavings > 50 ? green : red);

        // ── Highlight repaints toggle (footer) ──
        float ty = winH - 46.0f;
        drawCard(r, cardX, ty, cardW, 30);
        float lbl[4] = {0.80f, 0.82f, 0.89f, 1.0f};
        drawTextAt(r, "Highlight repaints", px + 22, ty + 8.0f, lbl, 11.0f, "normal");
        drawSwitch(r, px + pw - 58.0f, ty + 6.0f, m_highlightRepaints);
    }

    void drawRendererCard(GLRenderer& r, float px, float y0, float pw) {
        bool isForge = activeRenderMode() == RenderMode::Forge;
        drawCard(r, px + 10, y0, pw - 20, 96);

        drawSectionLabel(r, px + 22, y0 + 6, "RENDERER");

        // Active renderer label
        float lbl[4] = {0.55f, 0.57f, 0.66f, 1.0f};
        drawTextAt(r, "Active renderer", px + 22, y0 + 28, lbl, 11.0f, "normal");

        // Status pill
        const char* name = isForge ? "Forge" : "Flash";
        float tw = r.measureTextWidth(name, 11.0f, "bold");
        float pillW = tw + 20.0f;
        float pillX = px + pw - 22.0f - pillW;
        float pillBg[4];
        if (isForge) {
            pillBg[0] = 0.486f; pillBg[1] = 0.416f; pillBg[2] = 0.961f; pillBg[3] = 1.0f;
        } else {
            pillBg[0] = 0.10f; pillBg[1] = 0.42f; pillBg[2] = 0.42f; pillBg[3] = 1.0f;
        }
        r.drawRoundedRect(pillX, y0 + 27, pillW, 20, 10, pillBg);
        float pillText[4] = {1.0f, 1.0f, 1.0f, 1.0f};
        drawTextAt(r, name, pillX + 10, y0 + 30, pillText, 11.0f, "bold");

#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
        // Segmented Flash | Forge control
        float segY = y0 + 52.0f;
        drawRendererSegmented(r, px, segY, pw, isForge);
        const char* desc = isForge ? "Damage-limited retained-FBO" : "Full-frame rasterizer";
        float descCol[4] = {0.46f, 0.49f, 0.59f, 1.0f};
        drawTextAt(r, desc, px + 22, segY + 34.0f, descCol, 9.5f, "normal");
#else
        const char* fixed = isForge ? "Forge  (compile-time)" : "Flash  (compile-time)";
        float descCol[4] = {0.46f, 0.49f, 0.59f, 1.0f};
        drawTextAt(r, fixed, px + 22, y0 + 60.0f, descCol, 10.0f, "normal");
#endif
    }

    void drawRendererSegmented(GLRenderer& r, float px, float y, float pw, bool isForge) {
        float cW = pw - 20, h = 28.0f;
        float cBg[4] = {0.035f, 0.039f, 0.055f, 1.0f};
        r.drawRoundedRect(px + 10, y, cW, h, 7, cBg);

        float half = (cW - 6.0f) * 0.5f;
        const char* labels[2] = {"Flash", "Forge"};
        for (int i = 0; i < 2; i++) {
            bool sel = (i == 0) ? !isForge : isForge;
            float bx = px + 10 + 3.0f + half * i;
            if (sel) {
                float pill[4];
                if (i == 0) {
                    pill[0] = 0.10f; pill[1] = 0.42f; pill[2] = 0.42f; pill[3] = 1.0f;
                } else {
                    pill[0] = 0.486f; pill[1] = 0.416f; pill[2] = 0.961f; pill[3] = 1.0f;
                }
                r.drawRoundedRect(bx, y + 3, half, h - 6, 5, pill);
            }
            float tw = r.measureTextWidth(labels[i], 11.0f, "bold");
            float tc[4];
            if (sel) {
                tc[0] = 1.0f; tc[1] = 1.0f; tc[2] = 1.0f; tc[3] = 1.0f;
            } else {
                tc[0] = 0.45f; tc[1] = 0.48f; tc[2] = 0.58f; tc[3] = 1.0f;
            }
            drawTextAt(r, labels[i], bx + (half - tw) * 0.5f, y + 6.0f, tc, 11.0f, "bold");
        }
    }

    float logViewTop() const {
        if (m_activeTab == 2) {
            // Network list starts below toolbar+summary card; detail view
            // starts below its toolbar row.
            return netDetailOpen() ? (kContentY + 30.0f) : (kContentY + 56.0f);
        }
        return kContentY + 30.0f;
    }

    bool netDetailOpen() const { return m_selectedNetId != 0; }

    bool netDetailEntry(int id, DevNetEntry& out) const {
        if (id == 0) return false;
        auto snap = devNetSnapshot();
        for (auto& e : snap) {
            if (e.id == id) {
                out = e;
                return true;
            }
        }
        return false;
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
        // ── Toolbar: title + Clear button ──
        drawSectionLabel(r, px + 22, y0, "MESSAGES");
        float clearBg[4] = {0.10f, 0.11f, 0.14f, 1.0f};
        float clearBorder[4] = {0.17f, 0.19f, 0.24f, 1.0f};
        r.drawBorderedRoundedRect(px + pw - 78.0f, y0, 66.0f, 22.0f, 6, clearBg, 1.0f, clearBorder);
        float clearCol[4] = {0.72f, 0.74f, 0.82f, 1.0f};
        drawTextAt(r, "Clear", px + pw - 64.0f, y0 + 5.0f, clearCol, 10.0f, "bold");

        float top = y0 + 30.0f;
        float bottom = winH - 8.0f;
        float viewH = bottom - top;
        auto entries = devLogSnapshot();

        float textX = px + 66.0f;
        float textMaxW = px + pw - 16.0f - textX;
        std::vector<std::vector<std::string>> wrapped;
        wrapped.reserve(entries.size());
        float contentH = 4.0f;
        for (auto& e : entries) {
            auto lines = wrapLogText(r, e.msg, textMaxW, 10.0f);
            contentH += lines.size() * 16.0f;
            wrapped.push_back(std::move(lines));
        }
        m_logScroll.contentH = contentH;
        m_logScroll.viewH = viewH;
        if (contentH > viewH && m_logScroll.scroll > contentH - viewH)
            m_logScroll.scroll = contentH - viewH;
        if (m_logScroll.scroll < 0.0f) m_logScroll.scroll = 0.0f;

        // ── Scrollable content ──
        r.beginClip(px, top, pw, viewH);

        float colInfo[4]  = {0.62f, 0.64f, 0.72f, 1.0f};
        float colOk[4]    = {0.20f, 0.80f, 0.55f, 1.0f};
        float colWarn[4]  = {0.95f, 0.72f, 0.24f, 1.0f};
        float colErr[4]   = {0.95f, 0.34f, 0.26f, 1.0f};
        float colTime[4]  = {0.44f, 0.46f, 0.55f, 1.0f};
        float rowBg[4]    = {0.05f, 0.055f, 0.075f, 0.6f};

        float lineY = top - m_logScroll.scroll + 2.0f;
        for (size_t k = 0; k < entries.size(); k++) {
            auto& e = entries[k];
            auto& lines = wrapped[k];
            float entryH = lines.size() * 16.0f;
            if (lineY > bottom) break;
            if (lineY + entryH < top) { lineY += entryH; continue; }

            float* col = colInfo;
            switch (e.level) {
                case LOG_OK:    col = colOk; break;
                case LOG_WARN:  col = colWarn; break;
                case LOG_ERROR: col = colErr; break;
            }

            // Row background + level bar
            r.drawRect(px + 8, lineY, pw - 26, entryH, rowBg);
            r.drawRect(px + 8, lineY, 2.5f, entryH, col);

            for (size_t li = 0; li < lines.size(); li++) {
                if (li == 0)
                    drawTextAt(r, logTimestamp(e.time), px + 16, lineY, colTime, 10.0f, "normal");
                drawTextAt(r, lines[li], textX, lineY, col, 10.0f, "normal");
                lineY += 16.0f;
            }
        }

        r.endClip();

        // ── Scrollbar ──
        if (contentH > viewH) {
            float trackBg[4] = {0.055f, 0.061f, 0.082f, 1.0f};
            float thumbCol[4] = {0.35f, 0.37f, 0.45f, 1.0f};
            float trackX = px + pw - 10.0f;
            r.drawRect(trackX, top, 6.0f, viewH, trackBg);
            float thumbH = std::max(24.0f, (viewH / contentH) * viewH);
            float maxScroll = contentH - viewH;
            float thumbY = top + (m_logScroll.scroll / maxScroll) * (viewH - thumbH);
            r.drawRoundedRect(trackX, thumbY, 6.0f, thumbH, 3.0f, thumbCol);
        }
    }

    // ── Network tab ──
    static std::string fmtBytes(size_t b) {
        char buf[32];
        if (b >= 1048576)
            snprintf(buf, sizeof(buf), "%.1fMB", (double)b / 1048576.0);
        else if (b >= 1024)
            snprintf(buf, sizeof(buf), "%.1fKB", (double)b / 1024.0);
        else
            snprintf(buf, sizeof(buf), "%zuB", b);
        return buf;
    }

    static std::string fmtMs(double s) {
        char buf[32];
        if (s <= 0.0)
            snprintf(buf, sizeof(buf), "--");
        else if (s < 1.0)
            snprintf(buf, sizeof(buf), "%.0fms", s * 1000.0);
        else
            snprintf(buf, sizeof(buf), "%.1fs", s);
        return buf;
    }

    static std::string truncateText(GLRenderer& r, const std::string& s,
                                    float maxW, float fontSize) {
        if (r.measureTextWidth(s, fontSize, "normal") <= maxW) return s;
        std::string t = s;
        const std::string ell = "\xE2\x80\xA6";
        while (t.size() > 1) {
            t.pop_back();
            if (r.measureTextWidth(t + ell, fontSize, "normal") <= maxW) break;
        }
        return t + ell;
    }

    void drawNetworkTab(GLRenderer& r, float px, float y0, float pw, float winH) {
        if (netDetailOpen()) {
            DevNetEntry e;
            if (netDetailEntry(m_selectedNetId, e)) {
                drawNetDetails(r, px, y0, pw, winH, e);
                return;
            }
            m_selectedNetId = 0; // entry evicted from the ring buffer
        }

        // ── Toolbar ──
        drawSectionLabel(r, px + 22, y0, "REQUESTS");
        float clearBg[4] = {0.10f, 0.11f, 0.14f, 1.0f};
        float clearBorder[4] = {0.17f, 0.19f, 0.24f, 1.0f};
        r.drawBorderedRoundedRect(px + pw - 78.0f, y0, 66.0f, 22.0f, 6, clearBg, 1.0f, clearBorder);
        float clearCol[4] = {0.72f, 0.74f, 0.82f, 1.0f};
        drawTextAt(r, "Clear", px + pw - 64.0f, y0 + 5.0f, clearCol, 10.0f, "bold");

        // ── Summary card ──
        auto entries = devNetSnapshot();
        int total = (int)entries.size();
        int failed = 0;
        size_t bytes = 0;
        for (auto& e : entries) {
            if (e.error.size() || (e.done && e.status == 0)) failed++;
            if (e.done) bytes += e.bytes;
        }
        int ok = total - failed;

        char buf[128];
        drawCard(r, px + 10, y0 + 26, pw - 20, 26);
        float sumVal[4] = {0.80f, 0.82f, 0.89f, 1.0f};
        float sumErr[4] = {0.95f, 0.34f, 0.26f, 1.0f};
        snprintf(buf, sizeof(buf), "%d req", total);
        drawTextAt(r, buf, px + 20, y0 + 32, sumVal, 10.0f, "bold");
        snprintf(buf, sizeof(buf), "%d ok", ok);
        drawTextAt(r, buf, px + 84, y0 + 32, sumVal, 10.0f, "bold");
        snprintf(buf, sizeof(buf), "%d err", failed);
        drawTextAt(r, buf, px + 142, y0 + 32, failed ? sumErr : sumVal, 10.0f, "bold");
        std::string tot = fmtBytes(bytes);
        float totW = r.measureTextWidth(tot, 10.0f, "bold");
        drawTextAt(r, tot, px + pw - 20.0f - totW, y0 + 32, sumVal, 10.0f, "bold");

        // ── Request list ──
        float top = logViewTop();
        float listBottom = winH - 8.0f;
        float listH = listBottom - top;
        float rowH = 24.0f;
        float contentH = total * rowH;

        m_netScroll.contentH = contentH;
        m_netScroll.viewH = listH;
        if (contentH > listH && m_netScroll.scroll > contentH - listH)
            m_netScroll.scroll = contentH - listH;
        if (m_netScroll.scroll < 0.0f) m_netScroll.scroll = 0.0f;

        r.beginClip(px, top, pw, listH);

        if (total == 0) {
            float hintCol[4] = {0.46f, 0.49f, 0.59f, 1.0f};
            drawTextAt(r, "No network requests yet", px + 22, top + 16, hintCol, 11.0f, "normal");
            drawTextAt(r, "Requests made with fetch()", px + 22, top + 33, hintCol, 10.0f, "normal");
            drawTextAt(r, "will appear here", px + 22, top + 50, hintCol, 10.0f, "normal");
        } else {
            float colStatus[4]  = {0.75f, 0.78f, 0.85f, 1.0f};
            float colPending[4] = {0.44f, 0.46f, 0.55f, 1.0f};
            float colErr[4]     = {0.95f, 0.34f, 0.26f, 1.0f};
            float colGreen[4]   = {0.12f, 0.79f, 0.54f, 1.0f};
            float colBlue[4]    = {0.30f, 0.65f, 1.0f, 1.0f};
            float colOrange[4]  = {0.95f, 0.60f, 0.20f, 1.0f};
            float colMethod[4]  = {0.49f, 0.71f, 0.96f, 1.0f};
            float colUrl[4]     = {0.83f, 0.85f, 0.91f, 1.0f};
            float colDim[4]     = {0.44f, 0.46f, 0.55f, 1.0f};
            float rowBg[4]      = {0.05f, 0.055f, 0.075f, 0.5f};

            float contentX = px + 56.0f;
            float rightEnd = px + pw - 84.0f;
            float rowY = top - m_netScroll.scroll;

            for (auto& e : entries) {
                if (rowY > listBottom) break;
                if (rowY + rowH < top) { rowY += rowH; continue; }

                r.drawRect(px + 8, rowY, pw - 26, rowH - 2.0f, rowBg);

                // Status dot
                float* sCol = colPending;
                bool bad = (e.error.size() || (e.done && e.status == 0));
                if (bad) sCol = colErr;
                else if (!e.done) sCol = colPending;
                else if (e.status >= 200 && e.status < 300) sCol = colGreen;
                else if (e.status >= 300 && e.status < 400) sCol = colBlue;
                else if (e.status >= 400 && e.status < 500) sCol = colOrange;
                else if (e.status >= 500) sCol = colErr;
                r.drawRoundedRect(px + 14, rowY + 8, 8, 8, 4, sCol);

                // Status code
                float* codeCol = bad ? colErr : (e.status == 0 ? colPending : colStatus);
                std::string code = e.status ? std::to_string(e.status) : (e.done ? "--" : "...");
                drawTextAt(r, code, px + 28, rowY + 6.0f, codeCol, 10.0f, "bold");

                // Method + URL (flow together so they can't overlap)
                float methodW = r.measureTextWidth(e.method, 10.0f, "bold");
                float urlX = contentX + methodW + 6.0f;
                float urlMaxW = rightEnd - urlX - 2.0f;
                drawTextAt(r, e.method, contentX, rowY + 6.0f, colMethod, 10.0f, "bold");
                std::string url = truncateText(r, e.url, urlMaxW, 10.0f);
                drawTextAt(r, url, urlX, rowY + 6.0f, colUrl, 10.0f, "normal");

                // Duration + size (right-aligned)
                std::string dur = fmtMs(e.done ? e.duration : 0.0);
                std::string sz = e.done ? fmtBytes(e.bytes) : "--";
                float durW = r.measureTextWidth(dur, 9.0f, "normal");
                float szW = r.measureTextWidth(sz, 9.0f, "normal");
                drawTextAt(r, dur, px + pw - 14.0f - durW, rowY + 7.0f, colDim, 9.0f, "normal");
                drawTextAt(r, sz, px + pw - 20.0f - durW - szW, rowY + 7.0f, colDim, 9.0f, "normal");

                rowY += rowH;
            }
        }

        r.endClip();

        // ── Scrollbar ──
        if (contentH > listH) {
            float trackBg[4] = {0.055f, 0.061f, 0.082f, 1.0f};
            float thumbCol[4] = {0.35f, 0.37f, 0.45f, 1.0f};
            float trackX = px + pw - 10.0f;
            r.drawRect(trackX, top, 6.0f, listH, trackBg);
            float thumbH = std::max(24.0f, (listH / contentH) * listH);
            float maxScroll = contentH - listH;
            float thumbY = top + (m_netScroll.scroll / maxScroll) * (listH - thumbH);
            r.drawRoundedRect(trackX, thumbY, 6.0f, thumbH, 3.0f, thumbCol);
        }
    }

    void drawNetDetails(GLRenderer& r, float px, float y0, float pw, float winH,
                        const DevNetEntry& e) {
        // ── Toolbar: Back button + status ──
        float bbBg[4] = {0.10f, 0.11f, 0.14f, 1.0f};
        float bbBorder[4] = {0.17f, 0.19f, 0.24f, 1.0f};
        r.drawBorderedRoundedRect(px + 10, y0, 64.0f, 22.0f, 6, bbBg, 1.0f, bbBorder);
        float bbCol[4] = {0.72f, 0.74f, 0.82f, 1.0f};
        drawTextAt(r, "\xC2\xAB Back", px + 20, y0 + 5.0f, bbCol, 10.0f, "bold");

        float* stCol;
        bool bad = (e.error.size() || (e.done && e.status == 0));
        float stRed[4]   = {0.95f, 0.34f, 0.26f, 1.0f};
        float stGreen[4] = {0.12f, 0.79f, 0.54f, 1.0f};
        float stGrey[4]  = {0.62f, 0.64f, 0.72f, 1.0f};
        stCol = bad ? stRed : (e.status == 0 ? stGrey : stGreen);
        std::string status = e.error.empty()
            ? std::to_string(e.status)
            : "ERR";
        drawTextAt(r, status, px + 86, y0 + 5.0f, stCol, 10.0f, "bold");

        float top = logViewTop();
        float bottom = winH - 8.0f;
        float viewH = bottom - top;

        // ── Build display content ──
        float cardX = px + 10, cardW = pw - 20;
        float textX = px + 22, textMaxW = px + pw - 12.0f - textX;
        float valCol[4] = {0.83f, 0.85f, 0.91f, 1.0f};
        float dim[4]    = {0.55f, 0.57f, 0.66f, 1.0f};

        char buf[160];
        std::vector<std::vector<std::string>> generalLines;
        {
            snprintf(buf, sizeof(buf), "URL   %s", e.url.c_str());
            generalLines.push_back(wrapLogText(r, buf, textMaxW, 10.0f));
            if (e.error.empty())
                snprintf(buf, sizeof(buf), "Status   %d", e.status);
            else
                snprintf(buf, sizeof(buf), "Status   %s", e.error.c_str());
            generalLines.push_back(wrapLogText(r, buf, textMaxW, 10.0f));
            snprintf(buf, sizeof(buf), "Method   %s", e.method.c_str());
            generalLines.push_back(wrapLogText(r, buf, textMaxW, 10.0f));
            snprintf(buf, sizeof(buf), "Size     %s", fmtBytes(e.bytes).c_str());
            generalLines.push_back(wrapLogText(r, buf, textMaxW, 10.0f));
            snprintf(buf, sizeof(buf), "Time     %s", fmtMs(e.duration).c_str());
            generalLines.push_back(wrapLogText(r, buf, textMaxW, 10.0f));
        }

        // Response + request headers as wrapped line blocks.
        auto headerLines = [&](const std::string& head) {
            std::vector<std::string> out;
            if (head.empty()) {
                out.push_back("(none captured)");
                return out;
            }
            std::string cur;
            for (char c : head) {
                if (c == '\n') {
                    if (!cur.empty() && cur.back() == '\r') cur.pop_back();
                    auto w = wrapLogText(r, cur, textMaxW, 10.0f);
                    for (auto& l : w) out.push_back(l);
                    cur.clear();
                } else {
                    cur += c;
                }
            }
            if (!cur.empty()) {
                auto w = wrapLogText(r, cur, textMaxW, 10.0f);
                for (auto& l : w) out.push_back(l);
            }
            return out;
        };
        auto respLines = headerLines(e.responseHeaders);
        auto reqLines = headerLines(e.requestHeaders);

        std::vector<std::string> bodyLines;
        if (e.bodyPreview.empty())
            bodyLines.push_back("(no body)");
        else
            bodyLines = wrapLogText(r, e.bodyPreview, textMaxW, 10.0f);

        // ── Measure content height ──
        auto countH = [](const std::vector<std::vector<std::string>>& g) {
            float h = 0.0f;
            for (auto& l : g) h += l.size() * 16.0f;
            return h;
        };
        float generalH = 26.0f + countH(generalLines);
        float respH    = 26.0f + respLines.size() * 16.0f;
        float reqH     = 26.0f + reqLines.size() * 16.0f;
        float bodyH    = 26.0f + bodyLines.size() * 16.0f;
        float contentH = 4.0f + generalH + 8.0f + respH + 8.0f + reqH + 8.0f + bodyH + 8.0f;

        m_netScroll.contentH = contentH;
        m_netScroll.viewH = viewH;
        if (contentH > viewH && m_netScroll.scroll > contentH - viewH)
            m_netScroll.scroll = contentH - viewH;
        if (m_netScroll.scroll < 0.0f) m_netScroll.scroll = 0.0f;

        // ── Draw ──
        r.beginClip(px, top, pw, viewH);
        float y = top - m_netScroll.scroll + 4.0f;

        drawCard(r, cardX, y, cardW, generalH);
        drawSectionLabel(r, px + 22, y + 6, "GENERAL");
        float ry = y + 26.0f;
        for (auto& lines : generalLines) {
            bool first = true;
            for (auto& l : lines) {
                float* c = first ? valCol : dim;
                drawTextAt(r, l, textX, ry, c, 10.0f, "normal");
                first = false;
                ry += 16.0f;
            }
        }
        y += generalH + 8.0f;

        drawCard(r, cardX, y, cardW, respH);
        drawSectionLabel(r, px + 22, y + 6, "RESPONSE HEADERS");
        ry = y + 26.0f;
        for (auto& l : respLines) {
            drawTextAt(r, l, textX, ry, valCol, 10.0f, "normal");
            ry += 16.0f;
        }
        y += respH + 8.0f;

        drawCard(r, cardX, y, cardW, reqH);
        drawSectionLabel(r, px + 22, y + 6, "REQUEST HEADERS");
        ry = y + 26.0f;
        for (auto& l : reqLines) {
            drawTextAt(r, l, textX, ry, valCol, 10.0f, "normal");
            ry += 16.0f;
        }
        y += reqH + 8.0f;

        drawCard(r, cardX, y, cardW, bodyH);
        drawSectionLabel(r, px + 22, y + 6, "BODY");
        ry = y + 26.0f;
        for (auto& l : bodyLines) {
            drawTextAt(r, l, textX, ry, valCol, 10.0f, "normal");
            ry += 16.0f;
        }
        y += bodyH + 8.0f;

        r.endClip();

        // ── Scrollbar ──
        if (contentH > viewH) {
            float trackBg[4] = {0.055f, 0.061f, 0.082f, 1.0f};
            float thumbCol[4] = {0.35f, 0.37f, 0.45f, 1.0f};
            float trackX = px + pw - 10.0f;
            r.drawRect(trackX, top, 6.0f, viewH, trackBg);
            float thumbH = std::max(24.0f, (viewH / contentH) * viewH);
            float maxScroll = contentH - viewH;
            float thumbY = top + (m_netScroll.scroll / maxScroll) * (viewH - thumbH);
            r.drawRoundedRect(trackX, thumbY, 6.0f, thumbH, 3.0f, thumbCol);
        }
    }

    void drawNodeInfo(GLRenderer& r, float px, float y0, float pw, MorphNode* n) {
        if (!n) return;
        auto& s = n->style;

        float cardX = px + 10, cardW = pw - 20;
        float valCol[4] = {0.83f, 0.85f, 0.91f, 1.0f};
        float white[4] = {0.95f, 0.95f, 0.98f, 1.0f};
        float y = y0;
        char buf[160];

        // ── ELEMENT card ──
        float cardH = 88.0f;
        drawCard(r, cardX, y, cardW, cardH);
        drawSectionLabel(r, px + 22, y + 6, "ELEMENT");

        std::string tag;
        if (n->type == "__text__") {
            tag = "text";
        } else {
            tag = n->type.empty() ? "div" : n->type;
        }
        float badgeW = r.measureTextWidth("<" + tag + ">", 11.0f, "bold") + 16.0f;
        float badgeBg[4] = {0.19f, 0.17f, 0.36f, 1.0f};
        float badgeBorder[4] = {0.486f, 0.416f, 0.961f, 0.55f};
        r.drawBorderedRoundedRect(px + 22, y + 22, badgeW, 22, 6, badgeBg, 1.0f, badgeBorder);
        drawTextAt(r, "<" + tag + ">", px + 30, y + 26, white, 11.0f, "bold");

        // Clear selection button
        if (n == selectedNode) {
            float cbBg[4] = {0.11f, 0.12f, 0.15f, 1.0f};
            r.drawRoundedRect(px + pw - 44.0f, y + 22, 24, 22, 6, cbBg);
            float cbCol[4] = {0.75f, 0.45f, 0.45f, 1.0f};
            drawTextAt(r, "\xC3\x97", px + pw - 36.0f, y + 25, cbCol, 14.0f, "bold");
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
        float trailCol[4] = {0.62f, 0.64f, 0.72f, 1.0f};
        drawTextAt(r, trail, px + 22, y + 52, trailCol, 9.0f, "normal");

        std::string idc;
        if (!n->nodeId.empty()) idc += "#" + n->nodeId;
        if (!n->className.empty()) idc += "." + n->className;
        float idCol[4] = {0.55f, 0.57f, 0.66f, 1.0f};
        drawTextAt(r, idc.empty() ? "div" : idc, px + 22, y + 68, idCol, 10.0f, "normal");
        y += cardH + 8.0f;

        // ── LAYOUT card ──
        cardH = 34.0f + 4 * 17.0f;
        drawCard(r, cardX, y, cardW, cardH);
        drawSectionLabel(r, px + 22, y + 6, "LAYOUT");
        float ry = y + 26.0f;
        snprintf(buf, sizeof(buf), "%.0f \xC3\x97 %.0f", n->w, n->h);
        drawRow(r, px + 22, ry, "Size", buf, valCol); ry += 17.0f;
        snprintf(buf, sizeof(buf), "(%.0f, %.0f)", n->x, n->y);
        drawRow(r, px + 22, ry, "Position", buf, valCol); ry += 17.0f;
        snprintf(buf, sizeof(buf), "T:%.0f R:%.0f B:%.0f L:%.0f",
                 n->m_computedMargin[0], n->m_computedMargin[1],
                 n->m_computedMargin[2], n->m_computedMargin[3]);
        drawRow(r, px + 22, ry, "Margin", buf, valCol); ry += 17.0f;
        snprintf(buf, sizeof(buf), "T:%.0f R:%.0f B:%.0f L:%.0f",
                 s.padding[0], s.padding[1], s.padding[2], s.padding[3]);
        drawRow(r, px + 22, ry, "Padding", buf, valCol);
        y += cardH + 8.0f;

        // ── DISPLAY card ──
        cardH = 34.0f + 3 * 17.0f;
        drawCard(r, cardX, y, cardW, cardH);
        drawSectionLabel(r, px + 22, y + 6, "DISPLAY");
        ry = y + 26.0f;
        drawRow(r, px + 22, ry, "Display", s.display.c_str(), valCol); ry += 17.0f;
        drawRow(r, px + 22, ry, "Overflow", s.overflow.c_str(), valCol); ry += 17.0f;
        drawRow(r, px + 22, ry, "Box Sizing", s.boxSizing.c_str(), valCol);
        y += cardH + 8.0f;

        // ── STYLE card ──
        cardH = 34.0f + 5 * 17.0f;
        drawCard(r, cardX, y, cardW, cardH);
        drawSectionLabel(r, px + 22, y + 6, "STYLE");
        ry = y + 26.0f;

        float lbl[4] = {0.55f, 0.57f, 0.66f, 1.0f};
        float swatchX = px + pw - 34.0f;

        drawTextAt(r, "Color", px + 22, ry, lbl, 11.0f, "normal");
        drawSwatch(r, swatchX, ry + 1, s.color);
        formatColor(buf, sizeof(buf), s.color);
        drawTextAt(r, buf, px + 112, ry, valCol, 11.0f, "normal");
        ry += 17.0f;

        drawTextAt(r, "Background", px + 22, ry, lbl, 11.0f, "normal");
        drawSwatch(r, swatchX, ry + 1, s.bgColor);
        formatColor(buf, sizeof(buf), s.bgColor);
        drawTextAt(r, buf, px + 112, ry, valCol, 11.0f, "normal");
        ry += 17.0f;

        snprintf(buf, sizeof(buf), "%.0fpx", s.fontSize);
        drawRow(r, px + 22, ry, "Font Size", buf, valCol); ry += 17.0f;
        drawRow(r, px + 22, ry, "Weight", s.fontWeight.c_str(), valCol); ry += 17.0f;
        drawRow(r, px + 22, ry, "Align", s.textAlign.c_str(), valCol);
    }
};
