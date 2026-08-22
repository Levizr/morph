#pragma once
#include "../core/node.h"
#include "../core/event.h"
#include "../core/clipboard.h"
#include "radius.h"
#include <unordered_set>
#include <algorithm>
#include <cstring>
#include <cmath>

class InputNode : public MorphNode {
public:
    // ── Browser-parity attributes (always present — a few bytes) ──
    std::string value;
    std::string placeholder;
    // HTML spec: maxlength counts UTF-16 code units; when absent the
    // browser hard cap applies (524288 = 512K code units).
    int maxLength = 524288;
    int minLength = -1;          // -1 = unset; validation-only, never blocks typing
    bool disabled = false;
    std::string inputType = "text";   // "text" | "password"

    static constexpr int BROWSER_MAX_LENGTH = 524288;
    // Horizontal content padding shared by layout, rendering and hit-testing.
    static constexpr float kPadX = 10.0f;
    // Max bytes of clipboard we will even consider (max code units * max 4 bytes/char).
    static constexpr size_t kMaxClipboardBytes = (size_t)BROWSER_MAX_LENGTH * 4;

    // ── Lifetime tracking for reentrancy UAF guard ──────────────────
    // JS callbacks (onInput/onChange) may delete `this` (e.g. parent.removeChild).
    // After any callback we must not touch `this` without checking liveness.
    static std::unordered_set<InputNode*>& liveSet() {
        static std::unordered_set<InputNode*> s;
        return s;
    }
    static bool isAlive(InputNode* p) { return liveSet().find(p) != liveSet().end(); }

    ~InputNode() override {
        liveSet().erase(this);
        if (MorphNode::s_mouseCapture == this) MorphNode::s_mouseCapture = nullptr;
        if (MorphNode::s_focusedNode == this) MorphNode::s_focusedNode = nullptr;
        // Invalidate any dangling renderer pointer held by this node.
        m_lastRenderer = nullptr;
    }

    InputNode() {
        type = "input";
        style.cursor = "text";
        liveSet().insert(this);
    }

    // ── Helpers for sanitization / truncation ───────────────────────
    // Strip C0 controls, DEL, surrogates, >0x10FFFF and truncated sequences.
    static std::string sanitizeInputText(const std::string& s) {
        std::string out;
        out.reserve(s.size());
        for (size_t i = 0; i < s.size();) {
            unsigned char b = (unsigned char)s[i];
            int len = (b < 0x80) ? 1 : ((b >> 5) == 0x6) ? 2 : ((b >> 4) == 0xE) ? 3 : 4;
            if (i + len > s.size()) break;
            unsigned cp = 0xFFFD;
            switch (len) {
                case 1: cp = b; break;
                case 2: cp = ((b & 0x1F) << 6) | (s[i+1] & 0x3F); break;
                case 3: cp = ((b & 0x0F) << 12) | ((s[i+1] & 0x3F) << 6) | (s[i+2] & 0x3F); break;
                default: cp = ((b & 0x07) << 18) | ((s[i+1] & 0x3F) << 12) | ((s[i+2] & 0x3F) << 6) | (s[i+3] & 0x3F); break;
            }
            bool ok = cp >= 0x20 && cp != 0x7F && (cp < 0xD800 || cp > 0xDFFF) && cp <= 0x10FFFF;
            if (ok) out.append(s, i, len);
            i += len;
        }
        return out;
    }
    static std::string truncateUTF16(const std::string& s, int maxUnits) {
        if (maxUnits <= 0) return "";
        if ((int)s.size() <= maxUnits) { // fast path for ASCII: bytes == units
            // Still need to count astral (2 units) - check if any 4-byte seq
            bool hasAstral = false;
            for (unsigned char c : s) if (c >= 0xF0) { hasAstral = true; break; }
            if (!hasAstral) return s.substr(0, (size_t)maxUnits);
        }
        int units = 0;
        size_t pos = 0;
        for (size_t i = 0; i < s.size();) {
            unsigned char b = (unsigned char)s[i];
            int len = (b < 0x80) ? 1 : ((b >> 5) == 0x6) ? 2 : ((b >> 4) == 0xE) ? 3 : 4;
            if (i + len > s.size()) break;
            unsigned cp = 0xFFFD;
            switch (len) {
                case 1: cp = b; break;
                case 2: cp = ((b & 0x1F) << 6) | (s[i+1] & 0x3F); break;
                case 3: cp = ((b & 0x0F) << 12) | ((s[i+1] & 0x3F) << 6) | (s[i+2] & 0x3F); break;
                default: cp = ((b & 0x07) << 18) | ((s[i+1] & 0x3F) << 12) | ((s[i+2] & 0x3F) << 6) | (s[i+3] & 0x3F); break;
            }
            int need = (cp > 0xFFFF) ? 2 : 1;
            if (units + need > maxUnits) break;
            units += need;
            i += len;
            pos = i;
        }
        return s.substr(0, pos);
    }
    int effectiveMaxLength() const {
        if (maxLength < 0 || maxLength > BROWSER_MAX_LENGTH) return BROWSER_MAX_LENGTH;
        return maxLength;
    }

    void setValue(const std::string& v) {
        // Sanitize + cap to browser limit to prevent OOM via JS/IPC.
        std::string nv = sanitizeInputText(v);
        if (utf16Length(nv) > BROWSER_MAX_LENGTH) nv = truncateUTF16(nv, BROWSER_MAX_LENGTH);
        if (value == nv) return;
        try {
            value = std::move(nv);
        } catch (const std::bad_alloc&) {
            // Graceful truncate on allocation failure.
            try { value = truncateUTF16(sanitizeInputText(v), BROWSER_MAX_LENGTH); } catch (...) { value.clear(); }
        }
#ifdef MORPH_FEATURE_INPUT
        clampCaret();
        m_layoutValid = false;
        // Programmatic value changes invalidate the edit history.
        m_undoStack.clear(); m_redoStack.clear();
        m_lastEditKind = EditKind::None;
#endif
        markDirty(PaintDirty);
    }
    void setPlaceholder(const std::string& p) {
        std::string np = sanitizeInputText(p);
        if (utf16Length(np) > BROWSER_MAX_LENGTH) np = truncateUTF16(np, BROWSER_MAX_LENGTH);
        if (placeholder == np) return;
        try { placeholder = std::move(np); } catch (const std::bad_alloc&) { placeholder.clear(); }
#ifdef MORPH_FEATURE_INPUT
        m_layoutValid = false;
        markDirty(PaintDirty);
#endif
    }
    void setMaxLength(int ml) {
        int clamped = std::clamp(ml, 0, BROWSER_MAX_LENGTH);
        if (maxLength == clamped) return;
        maxLength = clamped;
#ifdef MORPH_FEATURE_INPUT
        // If current value exceeds new cap, truncate it.
        if (utf16Length(value) > maxLength) {
            try { value = truncateUTF16(value, maxLength); clampCaret(); } catch (...) {}
        }
        m_layoutValid = false;
        markDirty(PaintDirty);
#endif
    }

    // UTF-16 code-unit length of a UTF-8 string (HTML maxlength semantics:
    // BMP chars count 1, astral chars count 2).
    static int utf16Length(const std::string& s) {
        int units = 0;
        for (size_t i = 0; i < s.size();) {
            unsigned char b = (unsigned char)s[i];
            int len = (b < 0x80) ? 1 : ((b >> 5) == 0x6) ? 2 : ((b >> 4) == 0xE) ? 3 : 4;
            unsigned cp = 0xFFFD;
            if (i + len <= s.size()) {
                switch (len) {
                    case 1: cp = b; break;
                    case 2: cp = ((b & 0x1F) << 6) | (s[i+1] & 0x3F); break;
                    case 3: cp = ((b & 0x0F) << 12) | ((s[i+1] & 0x3F) << 6) | (s[i+2] & 0x3F); break;
                    default: cp = ((b & 0x07) << 18) | ((s[i+1] & 0x3F) << 12)
                                  | ((s[i+2] & 0x3F) << 6) | (s[i+3] & 0x3F); break;
                }
            }
            units += (cp > 0xFFFF) ? 2 : 1;
            i += len;
        }
        return units;
    }

#ifdef MORPH_FEATURE_INPUT
    // ── Editing state ────────────────────────────────────────────
    size_t caret = 0;            // insertion point (byte offset into value)
    size_t selAnchor = 0;        // selection anchor; range = [min, max] of {anchor, caret}
    float scrollX = 0;           // horizontal follow-scroll
    double m_caretTimer = 0.0;
    bool m_caretOn = true;
    bool m_dragSelecting = false;

    bool hasSelection() const { return selAnchor != caret; }
    size_t selStart() const { return selAnchor < caret ? selAnchor : caret; }
    size_t selEnd() const { return selAnchor < caret ? caret : selAnchor; }
    std::string selectedText() const { return value.substr(selStart(), selEnd() - selStart()); }

    // ── Undo / redo ──────────────────────────────────────────────
    struct EditSnapshot { std::string value; size_t caret; size_t selAnchor; };
    enum class EditKind { None, Type, Other };
    std::vector<EditSnapshot> m_undoStack, m_redoStack;
    EditKind m_lastEditKind = EditKind::None;
    float m_idleSinceEdit = 0.0f;
    bool m_wordDragMode = false;   // double-click then drag extends by words
    static constexpr size_t kMaxUndoDepth = 100;

    // Snapshots the current state before a mutation, starting a fresh undo
    // group. (Only insertText coalesces consecutive keystrokes; it manages
    // its own snapshots.) Fires no callbacks, so cannot kill `this`.
    bool pushUndo() {
        m_redoStack.clear();
        try {
            m_undoStack.push_back({value, caret, selAnchor});
        } catch (const std::bad_alloc&) {}
        if (m_undoStack.size() > kMaxUndoDepth)
            m_undoStack.erase(m_undoStack.begin());
        return true;
    }

    void applySnapshot(const EditSnapshot& s) {
        value = s.value;
        caret = s.caret;
        selAnchor = s.selAnchor;
        clampCaret();
        resetBlink();
        m_layoutValid = false;
        markDirty(PaintDirty);
    }

    void undo() {
        if (disabled || m_undoStack.empty()) return;
        EditSnapshot cur{value, caret, selAnchor};
        applySnapshot(m_undoStack.back());
        m_undoStack.pop_back();
        try { m_redoStack.push_back(std::move(cur)); } catch (...) {}
        m_lastEditKind = EditKind::None;
        fireValueEvents("change");
    }
    void redo() {
        if (disabled || m_redoStack.empty()) return;
        EditSnapshot cur{value, caret, selAnchor};
        applySnapshot(m_redoStack.back());
        m_redoStack.pop_back();
        try { m_undoStack.push_back(std::move(cur)); } catch (...) {}
        m_lastEditKind = EditKind::None;
        fireValueEvents("change");
    }

    // ── Word-boundary navigation ─────────────────────────────────
    static bool cpIsSpaceChar(unsigned cp) {
        return cp == ' ' || cp == '\t' || cp == '\v' || cp == '\f' ||
               cp == '\r' || cp == '\n' || cp == 0x85 || cp == 0xA0 ||
               cp == 0x1680 || (cp >= 0x2000 && cp <= 0x200A) ||
               cp == 0x2028 || cp == 0x2029 || cp == 0x202F ||
               cp == 0x205F || cp == 0x3000;
    }
    static bool cpIsAsciiPunct(unsigned cp) {
        return (cp >= 0x21 && cp <= 0x2F) || (cp >= 0x3A && cp <= 0x40) ||
               (cp >= 0x5B && cp <= 0x60) || (cp >= 0x7B && cp <= 0x7E);
    }
    static unsigned decodeCpAt(const std::string& s, size_t pos) {
        unsigned char b = (unsigned char)s[pos];
        int len = (b < 0x80) ? 1 : ((b >> 5) == 0x6) ? 2 : ((b >> 4) == 0xE) ? 3 : 4;
        if (pos + len > s.size()) return b;
        switch (len) {
            case 1: return b;
            case 2: return ((b & 0x1F) << 6) | (s[pos+1] & 0x3F);
            case 3: return ((b & 0x0F) << 12) | ((s[pos+1] & 0x3F) << 6) | (s[pos+2] & 0x3F);
            default: return ((b & 0x07) << 18) | ((s[pos+1] & 0x3F) << 12)
                          | ((s[pos+2] & 0x3F) << 6) | (s[pos+3] & 0x3F);
        }
    }
    // Word = maximal run of non-space chars; ASCII punctuation forms its own
    // run so Ctrl+Left stops between "foo.bar" segments (browser-like).
    size_t prevWordBoundary(size_t pos) {
        size_t p = pos > value.size() ? value.size() : pos;
        while (p > 0 && cpIsSpaceChar(decodeCpAt(value, prevBoundary(value, p))))
            p = prevBoundary(value, p);
        if (p == 0) return 0;
        bool wantPunct = cpIsAsciiPunct(decodeCpAt(value, prevBoundary(value, p)));
        size_t q = prevBoundary(value, p);
        while (q > 0) {
            size_t r = prevBoundary(value, q);
            unsigned c = decodeCpAt(value, r);
            if (cpIsSpaceChar(c) || cpIsAsciiPunct(c) != wantPunct) break;
            q = r;
        }
        return q;
    }
    size_t nextWordBoundary(size_t pos) {
        size_t p = pos >= value.size() ? value.size() : pos;
        while (p < value.size() && cpIsSpaceChar(decodeCpAt(value, p)))
            p = nextBoundary(value, p);
        if (p >= value.size()) return value.size();
        bool wantPunct = cpIsAsciiPunct(decodeCpAt(value, p));
        size_t q = nextBoundary(value, p);
        while (q < value.size()) {
            unsigned c = decodeCpAt(value, q);
            if (cpIsSpaceChar(c) || cpIsAsciiPunct(c) != wantPunct) break;
            q = nextBoundary(value, q);
        }
        return q;
    }

    // Case-insensitive key match without allocating a lowercased copy.
    static bool keyIs(const std::string& k, const char* lit) {
        size_t n = 0;
        for (; n < k.size() && lit[n]; n++)
            if (tolower((unsigned char)k[n]) != tolower((unsigned char)lit[n]))
                return false;
        return n == k.size() && lit[n] == '\0';
    }

    virtual bool onKeyEvent(const MorphEvent& e) override {
        if (disabled) return false;
        const std::string& k = e.key;
        bool ctrl = (e.mods & 0x02) != 0;   // GLFW_MOD_CONTROL
        bool shift = (e.mods & 0x01) != 0;  // GLFW_MOD_SHIFT

        if (keyIs(k, "backspace")) {
            bool hadSel = deleteSelection();
            if (!isAlive(this)) return true;
            if (!hadSel && caret > 0) {
                if (!pushUndo()) return true;
                size_t p = ctrl ? prevWordBoundary(caret) : prevBoundary(value, caret);
                value.erase(p, caret - p);
                caret = p;
                m_lastEditKind = EditKind::None;
                if (!valueChanged()) return true;
            }
            return true;
        }
        if (keyIs(k, "delete")) {
            bool hadSel = deleteSelection();
            if (!isAlive(this)) return true;
            if (!hadSel && caret < value.size()) {
                if (!pushUndo()) return true;
                size_t n = ctrl ? nextWordBoundary(caret) : nextBoundary(value, caret);
                value.erase(caret, n - caret);
                m_lastEditKind = EditKind::None;
                if (!valueChanged()) return true;
            }
            return true;
        }
        // Ctrl+Left/Right jump by whole words (Shift extends the selection).
        if (keyIs(k, "left")) {
            if (ctrl) { caret = prevWordBoundary(caret); if (!shift) selAnchor = caret; m_lastEditKind = EditKind::None; onEditVisualChange(); }
            else moveCaret(-1, shift);
            return true;
        }
        if (keyIs(k, "right")) {
            if (ctrl) { caret = nextWordBoundary(caret); if (!shift) selAnchor = caret; m_lastEditKind = EditKind::None; onEditVisualChange(); }
            else moveCaret(1, shift);
            return true;
        }
        if (keyIs(k, "home"))  { moveHomeEnd(0, shift); return true; }
        if (keyIs(k, "end"))   { moveHomeEnd(1, shift); return true; }
        // Single-line inputs: up/down jump to start/end (browser parity).
        if (keyIs(k, "up"))    { moveHomeEnd(0, shift); return true; }
        if (keyIs(k, "down"))  { moveHomeEnd(1, shift); return true; }
        if (keyIs(k, "enter") || keyIs(k, "kp_enter") || keyIs(k, "return")) {
            return true;   // single-line: consume, no newline
        }
        if (ctrl && !k.empty()) {
            if (keyIs(k, "a")) {
                selAnchor = 0; caret = value.size(); m_layoutValid = false;
                onEditVisualChange(); return true;
            }
            if (keyIs(k, "z")) { shift ? redo() : undo(); return true; }
            if (keyIs(k, "y")) { redo(); return true; }
            // Password fields never expose plaintext to the clipboard.
            if (keyIs(k, "c")) { if (inputType != "password") morph::setClipboard(selectedText()); return true; }
            if (keyIs(k, "x")) { if (inputType != "password") morph::setClipboard(selectedText()); deleteSelection(); if (!isAlive(this)) return true; return true; }
            if (keyIs(k, "v")) {
                std::string clip = morph::getClipboard();
                if (clip.size() > kMaxClipboardBytes) clip.resize(kMaxClipboardBytes);
                if (!clip.empty()) insertText(clip);
                // insertText may have deleted `this`.
                if (!isAlive(this)) return true;
                return true;
            }
        }
        return false;   // not consumed — global onKeyDown handlers still fire
    }

    virtual void onTextChar(unsigned int codepoint) override {
        // Reject C0 controls, DEL and UTF-16 surrogates (never valid scalars).
        if (disabled || codepoint < 0x20 || codepoint == 0x7F ||
            (codepoint >= 0xD800 && codepoint <= 0xDFFF))
            return;
        char buf[5];
        int len = utf8Encode(codepoint, buf);
        insertText(std::string(buf, len));
    }

    virtual bool onEvent(MorphEvent& e) override {
        // Release always runs, even if the field was disabled mid-drag —
        // otherwise the global mouse capture would stay stuck on this node.
        if (e.type == EventType::MouseUp) {
            m_dragSelecting = false;
            m_wordDragMode = false;
            if (MorphNode::s_mouseCapture == this)
                MorphNode::s_mouseCapture = nullptr;
        }
        if (!disabled) {
            if (e.type == EventType::MouseDown) {
                size_t p = charAtX(e.x);
                selAnchor = p;
                caret = p;
                m_dragSelecting = true;
                m_wordDragMode = false;
                // Capture the mouse: moves/releases outside the field keep
                // extending/ending the selection, browser-style.
                MorphNode::s_mouseCapture = this;
                onEditVisualChange();
            } else if (e.type == EventType::DoubleClick) {
                selectWordAt(charAtX(e.x));
                // Keep the drag alive: dragging after a double-click extends
                // the selection by whole words (browser behavior).
                m_dragSelecting = true;
                m_wordDragMode = true;
                onEditVisualChange();
            } else if (e.type == EventType::MouseMove && m_dragSelecting) {
                size_t p = charAtX(e.x);
                if (m_wordDragMode)
                    p = (p >= selAnchor) ? nextWordBoundary(p) : prevWordBoundary(p);
                if (p != caret) { caret = p; onEditVisualChange(); }
            }
        }
        return MorphNode::onEvent(e);
    }

    virtual void update(float dt) override {
        MorphNode::update(dt);
        m_idleSinceEdit += dt;
        if (!focused) { m_dragSelecting = false; return; }
        m_caretTimer += dt;
        if (m_caretTimer >= 0.53) {
            m_caretTimer -= 0.53;
            m_caretOn = !m_caretOn;
            markDirty(PaintDirty);
        }
    }

    // ── Cached text layout, shared by every render path ─────────
    // The flash compositor never calls draw()/executeDisplayList: it runs
    // recordDisplayList (display-list ops) + flattenExtra (text ops). So the
    // layout math (follow-scroll, positions) lives here and both paths read
    // the cached results.
    std::string m_dispCache;             // display text (password-masked)
    bool m_showPlaceholder = false;
    float m_textX = 0.0f, m_textY = 0.0f;
    float m_fsCache = 16.0f;
    std::string m_fwCache = "normal";
    float m_phColor[4] = {0.0f, 0.0f, 0.0f, 1.0f};
    // Edit overlay geometry (flatten-time absolute coords; -1 = hidden).
    // NaN = hidden sentinel (x can be legitimately negative when scrolled).
    float m_selX0 = NAN, m_selX1 = NAN, m_caretX = NAN;
    // Layout cache: recomputed only when an input to the layout changes.
    uint64_t m_layoutKey = 0;
    bool m_layoutValid = false;

    // FNV-1a fingerprint of everything computeTextLayout reads. Hashing is
    // orders of magnitude cheaper than the text shaping it guards against.
    uint64_t layoutKey() const {
        uint64_t h = 1469598103934665603ULL;
        auto mix = [&](const void* p, size_t n) {
            const unsigned char* b = (const unsigned char*)p;
            for (size_t i = 0; i < n; i++) { h ^= b[i]; h *= 1099511628211ULL; }
        };
        auto mixStr = [&](const std::string& s) {
            mix(s.data(), s.size());
            unsigned char sep = 1; mix(&sep, 1);
        };
        auto mixF = [&](float f) { uint32_t u; memcpy(&u, &f, 4); mix(&u, 4); };
        auto mixB = [&](bool b) { unsigned char v = b ? 1 : 0; mix(&v, 1); };
        mixStr(value); mixStr(placeholder); mixStr(inputType);
        mixF(x); mixF(y); mixF(w); mixF(h);
        mixF(scrollX);
        mixF(effFontSize());
        mixStr(effFontWeight());
        mix(style.color, sizeof(float) * 4);
        mixB(focused); mixB(disabled); mixB(m_caretOn);
        uint64_t c = (uint64_t)caret, a = (uint64_t)selAnchor;
        mix(&c, 8); mix(&a, 8);
        return h;
    }
    // Called when the Renderer is destroyed/recreated so dangling pointer is cleared.
    void invalidateRendererCache() { m_lastRenderer = nullptr; m_layoutValid = false; }

    void computeTextLayout(Renderer& r) {
        uint64_t key = layoutKey();
        if (m_layoutValid && key == m_layoutKey && m_lastRenderer == &r) return;
        m_lastRenderer = &r;
        m_layoutKey = key;
        m_layoutValid = true;

        m_dispCache.clear();
        m_showPlaceholder = false;
        m_selX0 = m_selX1 = m_caretX = NAN;
        float availW = w - 2.0f * kPadX;
        if (availW <= 0.0f) return;
        m_fsCache = effFontSize();
        m_fwCache = effFontWeight();
        m_showPlaceholder = value.empty() && !placeholder.empty();
        m_dispCache = displayText();

        const float* cc = style.color;
        m_phColor[0] = cc[0]; m_phColor[1] = cc[1];
        m_phColor[2] = cc[2]; m_phColor[3] = cc[3] * 0.45f;

        float textW = r.measureTextWidth(m_dispCache, m_fsCache, m_fwCache);
        float beforeW = r.measureTextWidth(displayPrefix(caret), m_fsCache, m_fwCache);

        // Horizontal follow-scroll keeps the caret inside the box.
        float maxScroll = textW > availW ? textW - availW : 0.0f;
        if (beforeW - scrollX > availW) scrollX = beforeW - availW;
        if (beforeW < scrollX) scrollX = beforeW;
        if (scrollX > maxScroll) scrollX = maxScroll;
        if (scrollX < 0) scrollX = 0;

        m_textX = x + kPadX - scrollX;
        // Line box shared by text, caret and selection highlight — the caret
        // matches the rendered font size (browser behavior).
        m_textY = y + (h - m_fsCache * 1.15f) * 0.5f;

        if (focused && !disabled) {
            if (hasSelection() && !m_dispCache.empty()) {
                m_selX0 = m_textX + r.measureTextWidth(displayPrefix(selStart()), m_fsCache, m_fwCache);
                m_selX1 = m_textX + r.measureTextWidth(displayPrefix(selEnd()), m_fsCache, m_fwCache);
            }
            if (m_caretOn)
                m_caretX = m_textX + beforeW;
        }
    }

    // Legacy immediate-mode path (window render() fallback): draws value/
    // placeholder text + selection highlight + caret directly, clipped to
    // the content box.
    void drawTextAndCaret(Renderer& r) {
        computeTextLayout(r);
        r.beginClip(x + kPadX, y, w - 2.0f * kPadX, h);

        if (!std::isnan(m_selX0) && m_selX1 != m_selX0) {
            float hl[4] = {0.20f, 0.56f, 1.0f, 0.35f};
            float left = std::min(m_selX0, m_selX1);
            float right = std::max(m_selX0, m_selX1);
            r.drawRect(left, m_textY, right - left,
                       m_fsCache * 1.15f, hl);
        }

        if (m_showPlaceholder) {
            r.drawText(placeholder, m_textX, m_textY, m_phColor,
                       TextAlign::Left, m_fsCache, m_fwCache);
        } else if (!m_dispCache.empty()) {
            r.drawText(m_dispCache, m_textX, m_textY, style.color,
                       TextAlign::Left, m_fsCache, m_fwCache);
        }

        // Caret: thin bar at the insertion point, blinking while focused,
        // sized to the rendered font's line box.
        if (!std::isnan(m_caretX))
            r.drawRect(m_caretX, m_textY, 2.0f, m_fsCache * 1.15f, style.color);

        r.endClip();
    }

    // Flash compositor path: text leaves the node as a FlatTextOp; selection
    // + caret ride FlatRenderNode overlay fields drawn by renderNode between
    // this node's background and its text, clipped to the content box.
    int flattenExtra(RenderFrame& frame, FlatRenderNode& fn) override {
        const char* txt = nullptr;
        const float* col = nullptr;
        if (m_showPlaceholder) { txt = placeholder.c_str(); col = m_phColor; }
        else if (!m_dispCache.empty()) { txt = m_dispCache.c_str(); col = style.color; }
        fn.clipText = true;
        fn.textClipPadX = kPadX;
        fn.caretY = m_textY;
        fn.caretH = m_fsCache * 1.15f;
        fn.selX0 = m_selX0;
        fn.selX1 = m_selX1;
        fn.caretX = m_caretX;
        { float sc[4] = {0.20f, 0.56f, 1.0f, 0.35f}; memcpy(fn.selColor, sc, sizeof(float) * 4); }
        memcpy(fn.caretColor, style.color, sizeof(float) * 4);
        if (!txt || !*txt) return 0;
        FlatTextOp fto;
        fto.nodeId = fn.id;
        fto.text = txt;
        fto.x = m_textX;
        fto.y = m_textY;
        fto.color[0] = col[0]; fto.color[1] = col[1];
        fto.color[2] = col[2]; fto.color[3] = col[3];
        fto.color[3] *= fn.opacity;   // group opacity, like TextNode
        fto.align = 0;                // TextAlign::Left
        fto.fontSize = m_fsCache;
        fto.fontWeight = (m_fwCache == "bold" || m_fwCache == "700" ||
                          m_fwCache == "800" || m_fwCache == "900") ? 1 : 0;
        frame.textOps.push_back(fto);
        return 1;
    }
#endif // MORPH_FEATURE_INPUT

    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? (style.borderRadius > 0.0f ? style.borderRadius : 6.0f) : snapRadius(style.borderRadius > 0.0f ? style.borderRadius : 6.0f);
        DrawOp bg;
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = m_isTransitioning ? style.borderWidth : snapBorderWidth(style.borderWidth);
            bool inner = (style.boxSizing == "border-box");
            if (inner)
                bg.setBordered(sx, sy, sw, sh, rad, style.bgColor, bw, style.borderColor);
            else
                bg.setBordered(sx - bw, sy - bw, sw + 2.0f * bw, sh + 2.0f * bw,
                               rad, style.bgColor, bw, style.borderColor);
        } else
#endif
        {
            bg.setRounded(sx, sy, sw, sh, rad, style.bgColor);
        }
        m_displayList.push_back(bg);

#ifdef MORPH_FEATURE_INPUT
        // Text layout cache for flattenExtra; selection/caret ride the
        // FlatRenderNode overlay fields (see flattenExtra).
        computeTextLayout(r);
#endif
    }

    void executeDisplayList(Renderer& r) override {
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? style.borderRadius : snapRadius(style.borderRadius);

#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, sx, sy);
#endif

        // 1. Render self (background)
        for (auto& op : m_displayList) {
            switch (op.type) {
                case DrawOp::Rect: r.drawRect(op.x,op.y,op.w,op.h,&op.r); break;
                case DrawOp::RoundedRect: r.drawRoundedRect(op.x,op.y,op.w,op.h,op.data[0],&op.r); break;
                case DrawOp::BorderedRect: r.drawBorderedRect(op.x,op.y,op.w,op.h,&op.r,op.data[1],&op.br); break;
                case DrawOp::BorderedRoundedRect: r.drawBorderedRoundedRect(op.x,op.y,op.w,op.h,op.data[0],&op.r,op.data[1],&op.br); break;
                default: break;
            }
        }

        // 2. Clip + text/caret + scroll + children
        bool needRadiusClip = rad > 0.0f;
#ifdef MORPH_FEATURE_SCROLL
        bool scrolling = scrollEnabled && contentH > sh;
        bool needRectClip = scrolling || style.overflow == "hidden" || style.overflow == "auto";
#else
        bool needRectClip = false;
#endif

        if (needRectClip || needRadiusClip) {
#ifdef MORPH_FEATURE_TRANSFORM
            if (needRectClip) {
                if (pushedSelf)
                    r.beginRoundedClip(sx, sy, sw, sh, 0.0f);
                else
                    r.beginClip(sx, sy, sw, sh);
            }
#else
            if (needRectClip) r.beginClip(sx, sy, sw, sh);
#endif
            if (needRadiusClip) r.beginRoundedClip(sx, sy, sw, sh, rad);
#ifdef MORPH_FEATURE_INPUT
            drawTextAndCaret(r);
#endif
#ifdef MORPH_FEATURE_SCROLL
            if (scrolling) r.pushScrollOffset(0, -scrollY);
#endif
            for (auto* child : children)
                child->executeDisplayList(r);
#ifdef MORPH_FEATURE_SCROLL
            if (scrolling) r.popScrollOffset(0, -scrollY);
#endif
            if (needRadiusClip) r.endRoundedClip();
            if (needRectClip) r.endClip();
        } else {
#ifdef MORPH_FEATURE_INPUT
            drawTextAndCaret(r);
#endif
            for (auto* child : children)
                child->executeDisplayList(r);
        }

#ifdef MORPH_FEATURE_SCROLL
        if (scrolling) drawScrollbar(r);
#endif

#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
#endif
    }

    void draw(Renderer& r) override {
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? (style.borderRadius > 0.0f ? style.borderRadius : 6.0f) : snapRadius(style.borderRadius > 0.0f ? style.borderRadius : 6.0f);
#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, sx, sy);
#endif
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = m_isTransitioning ? style.borderWidth : snapBorderWidth(style.borderWidth);
            bool inner = (style.boxSizing == "border-box");
            if (inner)
                r.drawBorderedRoundedRect(sx, sy, sw, sh, rad, style.bgColor,
                                          bw, style.borderColor);
            else
                r.drawBorderedRoundedRect(sx - bw, sy - bw,
                                          sw + 2.0f * bw, sh + 2.0f * bw,
                                          rad, style.bgColor,
                                          bw, style.borderColor);
        } else
#endif
            r.drawRoundedRect(sx, sy, sw, sh, rad, style.bgColor);
#ifdef MORPH_FEATURE_SCROLL
        if (scrollEnabled && contentH > sh) {
#ifdef MORPH_FEATURE_TRANSFORM
            if (pushedSelf)
                r.beginRoundedClip(sx, sy, sw, sh, 0.0f);
            else
                r.beginClip(sx, sy, sw, sh);
#else
            r.beginClip(sx, sy, sw, sh);
#endif
            r.pushScrollOffset(0, -scrollY);
#ifdef MORPH_FEATURE_INPUT
            drawTextAndCaret(r);
#endif
            for (auto* child : children) {
                float childVisY = child->y - scrollY;
                if (childVisY + child->h > sy && childVisY < sy + sh)
                    child->draw(r);
            }
            r.popScrollOffset(0, -scrollY);
            r.endClip();
            drawScrollbar(r);
        } else
#endif
        {
#ifdef MORPH_FEATURE_INPUT
            drawTextAndCaret(r);
#endif
            for (auto* child : children)
                child->draw(r);
        }

#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
#endif
    }

#ifdef MORPH_FEATURE_SCROLL
    void drawScrollbar(Renderer& r) {
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float sbw = m_isTransitioning ? style.scrollbarWidth : snapBorderWidth(style.scrollbarWidth);
        float trackX = sx + sw - sbw;
        r.drawRect(trackX, sy, sbw, sh, style.scrollbarTrackColor);
        float thumbH = sc((sh / contentH) * sh);
        float thumbY = sy + sc((scrollY / (contentH - sh)) * (sh - thumbH));
        if (thumbY < sy) thumbY = sy;
        if (thumbY + thumbH > sy + sh) thumbY = sy + sh - thumbH;
        float radius = m_isTransitioning ? style.scrollbarBorderRadius : snapRadius(style.scrollbarBorderRadius);
        if (radius > thumbH * 0.5f) radius = thumbH * 0.5f;
        if (radius < 0.5f) radius = 0.5f;
        r.drawRoundedRect(trackX, thumbY, sbw, thumbH, radius, style.scrollbarThumbColor);
    }
#endif

#ifdef MORPH_FEATURE_INPUT
private:
    static size_t prevBoundary(const std::string& s, size_t pos) {
        if (pos == 0 || pos > s.size()) return 0;
        size_t p = pos - 1;
        while (p > 0 && (s[p] & 0xC0) == 0x80) p--;
        return p;
    }
    static size_t nextBoundary(const std::string& s, size_t pos) {
        if (pos >= s.size()) return s.size();
        size_t p = pos + 1;
        while (p < s.size() && (s[p] & 0xC0) == 0x80) p++;
        return p;
    }

    void clampCaret() {
        if (caret > value.size()) caret = value.size();
        if (selAnchor > value.size()) selAnchor = value.size();
    }

    // Fires React-style per-edit events: onInput + onChange both carry the
    // current value (React normalizes change → input for text fields).
    // Returns false if `this` was deleted inside a callback.
    bool fireValueEvents(const char* tn) {
        JsObject evt;
        evt.set("value", JsString(value));
        evt.set("type", JsString(tn));
        if (onInput) onInput(evt);
        if (!isAlive(this)) return false;
        if (onChange) onChange(evt);
        if (!isAlive(this)) return false;
        return true;
    }

    bool valueChanged() {
        clampCaret();
        resetBlink();
        markDirty(PaintDirty);
        return fireValueEvents("change");
    }

    void onEditVisualChange() {
        resetBlink();
        markDirty(PaintDirty);
    }

    void resetBlink() { m_caretTimer = 0.0; m_caretOn = true; }

    void insertText(const std::string& text) {
        // Sanitize first (paste and script paths share the same policy).
        std::string clean;
        try { clean = sanitizeInputText(text); } catch (const std::bad_alloc&) { return; }
        if (clean.empty()) return;
        // Undo grouping decided BEFORE any mutation: single word-char
        // keystrokes coalesce (until 1s idle or a different edit kind);
        // pastes, spaces and punctuation start fresh groups.
        unsigned cp0 = decodeCpAt(clean, 0);
        bool typeGroup = clean.size() <= 4 && !cpIsSpaceChar(cp0) &&
                         !cpIsAsciiPunct(cp0) && m_lastEditKind != EditKind::Other;
        bool coalesce = typeGroup && m_idleSinceEdit < 1.0f;
        if (!coalesce) {
            m_redoStack.clear();
            try {
                m_undoStack.push_back({value, caret, selAnchor});
            } catch (const std::bad_alloc&) { return; }
            if (m_undoStack.size() > kMaxUndoDepth)
                m_undoStack.erase(m_undoStack.begin());
        }
        m_lastEditKind = typeGroup ? EditKind::Type : EditKind::Other;
        m_idleSinceEdit = 0.0f;
        // Replace selection without recording its own undo step — the
        // snapshot above already covers "text + selection" pre-state.
        deleteSelectionRaw();
        // Enforce maxlength in UTF-16 code units (browser semantics).
        int cap = effectiveMaxLength();
        int budget = cap - utf16Length(value);
        if (budget <= 0) { resetBlink(); markDirty(PaintDirty); return; }
        std::string accepted;
        accepted.reserve(std::min<size_t>(clean.size(), (size_t)budget * 4));
        int used = 0;
        for (size_t i = 0; i < clean.size();) {
            unsigned char b = (unsigned char)clean[i];
            int len = (b < 0x80) ? 1 : ((b >> 5) == 0x6) ? 2 : ((b >> 4) == 0xE) ? 3 : 4;
            if (i + len > clean.size()) break;
            unsigned cp = 0xFFFD;
            switch (len) {
                case 1: cp = b; break;
                case 2: cp = ((b & 0x1F) << 6) | (clean[i+1] & 0x3F); break;
                case 3: cp = ((b & 0x0F) << 12) | ((clean[i+1] & 0x3F) << 6) | (clean[i+2] & 0x3F); break;
                default: cp = ((b & 0x07) << 18) | ((clean[i+1] & 0x3F) << 12) | ((clean[i+2] & 0x3F) << 6) | (clean[i+3] & 0x3F); break;
            }
            int units = (cp > 0xFFFF) ? 2 : 1;
            if (used + units > budget) break;
            // Avoid extra alloc for single char: append slice directly.
            accepted.append(clean, i, len);
            used += units;
            i += len;
        }
        if (accepted.empty()) { resetBlink(); markDirty(PaintDirty); return; }
        try {
            value.insert(caret, accepted);
        } catch (const std::bad_alloc&) {
            // Try truncated fallback.
            try {
                std::string fallback = truncateUTF16(clean, budget);
                if (fallback.empty()) { resetBlink(); markDirty(PaintDirty); return; }
                value.insert(caret, fallback);
                accepted = std::move(fallback);
            } catch (...) { return; }
        }
        caret += accepted.size();
        selAnchor = caret;
        valueChanged();
    }

    // Deletes the selected range, recording an undo step first.
    bool deleteSelection() {
        if (!hasSelection()) return false;
        (void)pushUndo();
        m_lastEditKind = EditKind::Other;
        deleteSelectionRaw();
        return true;
    }

    // Raw deletion used by paths that already recorded an undo snapshot
    // (e.g. insertText replacing the selection).
    void deleteSelectionRaw() {
        if (!hasSelection()) return;
        value.erase(selStart(), selEnd() - selStart());
        caret = selStart();
        selAnchor = caret;
    }

    void moveCaret(int dir, bool extend) {
        caret = (dir < 0) ? prevBoundary(value, caret) : nextBoundary(value, caret);
        if (!extend) selAnchor = caret;
        onEditVisualChange();
    }

    void moveHomeEnd(bool end, bool extend) {
        caret = end ? value.size() : 0;
        if (!extend) selAnchor = caret;
        onEditVisualChange();
    }

    void selectWordAt(size_t pos) {
        if (value.empty()) { selAnchor = caret = 0; return; }
        if (pos >= value.size()) pos = value.size() - 1;
        auto isWord = [](char c) { return !isspace((unsigned char)c); };
        size_t s = pos, epos = pos;
        while (s > 0 && isWord(value[s]) && isWord(value[s - 1])) s--;
        while (epos < value.size() && isWord(value[epos])) epos++;
        selAnchor = s;
        caret = epos;
    }

    // Maps a click x-coordinate to the nearest caret byte offset.
    // O(n): accumulates one display-grapheme width at a time instead of
    // re-measuring the whole prefix per candidate. In password mode the
    // display is one bullet per value char, so value offsets advance in
    // lockstep with display graphemes.
    size_t charAtX(float clickX) {
        Renderer* r = m_lastRenderer;
        if (!r || m_dispCache.empty()) return 0;
        float relX = clickX - (x + kPadX) + scrollX;
        float fs = m_fsCache;
        const std::string& fw = m_fwCache;
        size_t best = 0;                       // boundary before first char
        float bestDist = relX < 0 ? -relX : relX;
        float acc = 0.0f;
        size_t vi = 0;                         // matching offset into value
        for (size_t di = 0; di < m_dispCache.size();) {
            unsigned char b = (unsigned char)m_dispCache[di];
            int dlen = (b < 0x80) ? 1 : ((b >> 5) == 0x6) ? 2 : ((b >> 4) == 0xE) ? 3 : 4;
            if (di + dlen > m_dispCache.size()) break;
            acc += r->measureTextWidth(m_dispCache.substr(di, dlen), fs, fw);
            di += dlen;
            vi = nextBoundary(value, vi);
            float d = acc - relX;
            if (d < 0) d = -d;
            if (d < bestDist) { bestDist = d; best = vi; }
        }
        return best;
    }

    // Password masking: one bullet per character, like browsers.
    std::string displayText() const {
        if (inputType != "password" || value.empty()) return value;
        std::string d;
        for (size_t i = 0; i < value.size();)
            d += "\xE2\x80\xA2", i = nextBoundary(value, i);
        return d;
    }
    // Display-layer prefix up to a byte offset in the value (for measuring).
    std::string displayPrefix(size_t bytePos) const {
        if (inputType != "password") return value.substr(0, bytePos);
        std::string d;
        for (size_t i = 0; i < bytePos && i <= value.size();) {
            d += "\xE2\x80\xA2";
            if (i >= value.size()) break;
            i = nextBoundary(value, i);
        }
        return d;
    }

    float effFontSize() const {
        if (style.fontSize != 16.0f || !parent) return style.fontSize;
        float pfs = parent->style.fontSize;
        return (pfs != 16.0f) ? pfs : style.fontSize;
    }
    const std::string& effFontWeight() const {
        if (style.fontWeight != "normal" || !parent) return style.fontWeight;
        return (parent->style.fontWeight != "normal") ? parent->style.fontWeight : style.fontWeight;
    }

    static int utf8Encode(unsigned int cp, char out[5]) {
        if (cp < 0x80) { out[0] = (char)cp; return 1; }
        if (cp < 0x800) {
            out[0] = (char)(0xC0 | (cp >> 6));
            out[1] = (char)(0x80 | (cp & 0x3F));
            return 2;
        }
        if (cp < 0x10000) {
            out[0] = (char)(0xE0 | (cp >> 12));
            out[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
            out[2] = (char)(0x80 | (cp & 0x3F));
            return 3;
        }
        out[0] = (char)(0xF0 | (cp >> 18));
        out[1] = (char)(0x80 | ((cp >> 12) & 0x3F));
        out[2] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[3] = (char)(0x80 | (cp & 0x3F));
        return 4;
    }

    // Renderer seen during the last paint — used for click-to-caret text
    // measurement outside the draw pass. Only valid between paint and the
    // next renderer teardown; guarded by the null check in charAtX.
    Renderer* m_lastRenderer = nullptr;
#endif // MORPH_FEATURE_INPUT
};
