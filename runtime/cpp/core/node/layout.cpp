#include "../node.h"
#include "../renderer.h"
#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <cstring>

// Padding/border allowance outside the content box, with relative units
// resolved against the child's own fonts (pctBase = CB width for the
// `%` fallback; border `%` is invalid CSS and never parses).
static float hBonus(const MorphNode* n, float pctBase, Renderer* r) {
#ifdef MORPH_FEATURE_BORDER_BOX
    if (n->style.boxSizing == CSS::BoxSizing::BorderBox) return 0.0f;
#endif
    UnitEnv env = const_cast<MorphNode*>(n)->unitEnv(pctBase, r);
    float pl = resolveUnits(n->style.padding[3], env);
    float pr = resolveUnits(n->style.padding[1], env);
#ifdef MORPH_FEATURE_BORDER
    return pl + pr + borderOuterH(n->style, env);
#else
    return pl + pr;
#endif
}

static float vBonus(const MorphNode* n, float pctBase, Renderer* r) {
#ifdef MORPH_FEATURE_BORDER_BOX
    if (n->style.boxSizing == CSS::BoxSizing::BorderBox) return 0.0f;
#endif
    UnitEnv env = const_cast<MorphNode*>(n)->unitEnv(pctBase, r);
    float pt = resolveUnits(n->style.padding[0], env);
    float pb = resolveUnits(n->style.padding[2], env);
#ifdef MORPH_FEATURE_BORDER
    return pt + pb + borderOuterV(n->style, env);
#else
    return pt + pb;
#endif
}

#ifdef MORPH_FEATURE_INLINE
// Translate every descendant by (dx, dy) so the subtree keeps the relative
// layout its own passes computed (inline runs stay side-by-side, blocks
// stay stacked). Used when a parent moves an item without re-laying it out.
static void shiftChildrenSubtree(MorphNode* n, float dx, float dy)
{
    for (auto* c : n->children)
    {
        c->x += dx;
        c->y += dy;
        shiftChildrenSubtree(c, dx, dy);
    }
}

// True for inline-level containers with no box decoration of their own
// (no background, border, padding, margins, explicit size, scrolling):
// browsers lay their children out in the parent flow instead of treating
// the box as atomic, so links and spans can break across lines mid-text.
static bool isTransparentInline(MorphNode* n)
{
    if (n->style.display != CSS::Display::Inline) return false;
    if (n->style.bgColor[3] != 0.0f) return false;
#ifdef MORPH_FEATURE_BORDER
    if (!n->style.borderWidth.isZero()) return false;
#endif
    if (!n->style.padding[0].isZero() || !n->style.padding[1].isZero()
        || !n->style.padding[2].isZero() || !n->style.padding[3].isZero())
        return false;
    if (!n->style.margin[0].isZero() || !n->style.margin[1].isZero()
        || !n->style.margin[2].isZero() || !n->style.margin[3].isZero())
        return false;
    if (n->style.explicitWidth.isSet() || n->style.explicitHeight.isSet()) return false;
    if (n->scrollEnabled) return false;
    if (n->style.overflow != CSS::Overflow::Visible) return false;
    for (auto* c : n->children)
    {
        if (c->style.display != CSS::Display::Inline
            && c->style.display != CSS::Display::InlineBlock
            && c->type != NodeType::Text && c->type != NodeType::Expr)
            return false;
    }
    return true;
}

// Push a node into the inline run, splicing transparent inline containers
// (links, spans) so their children flow directly: order preserved, tree
// untouched (paint, hit-testing and events still resolve through it).
// Spliced containers are recorded in unwrapped (pre-order); their boxes
// are rebuilt from children at the end of the run, so a later real layout
// of the container (dirty flags) repositions children identically instead
// of clobbering run positions with stale ones.
static void pushInlineRunNode(std::vector<MorphNode*>& run, MorphNode* n,
                              std::vector<MorphNode*>* unwrapped)
{
    if (isTransparentInline(n))
    {
        if (unwrapped) unwrapped->push_back(n);
        for (auto* c : n->children)
            pushInlineRunNode(run, c, unwrapped);
        return;
    }
    run.push_back(n);
}
#endif

// Display lists bake absolute coordinates, so any node moved without a full
// layout pass must repaint its subtree. Used by the inline positioner and
// the button label centering below (production has no DEV geometry diff).
static void markSubtreePaintDirty(MorphNode* n) {
    n->markDirty(PaintDirty);
    for (auto* c : n->children) markSubtreePaintDirty(c);
}

#ifdef MORPH_FEATURE_INLINE
// Split text into space/tab-separated words (browser line-breaking units).
// Newlines make a run unsplittable (TextNode wraps those itself); the
// caller checks for them. Splitting is deterministic so layout and paint
// agree without shared caches.
static std::vector<std::string> splitInlineWords(const std::string& text)
{
    std::vector<std::string> words;
    size_t i = 0;
    while (i < text.size())
    {
        while (i < text.size() && (text[i] == ' ' || text[i] == '\t')) i++;
        if (i >= text.size() || text[i] == '\n') break;
        size_t j = i;
        while (j < text.size() && text[j] != ' ' && text[j] != '\t' && text[j] != '\n') j++;
        words.push_back(text.substr(i, j - i));
        i = j;
    }
    return words;
}

static std::string joinInlineWords(
    const std::vector<std::string>& words, size_t start, size_t count)
{
    std::string out;
    for (size_t k = 0; k < count; k++)
    {
        if (k > 0) out += ' ';
        out += words[start + k];
    }
    return out;
}

// One entry of an inline run under layout: either a whole node or a word
// fragment slice of a split text run ([fw0, fw0 + fwn), painted as
// fragText). fwn == 0 means the whole node (legacy path, no fragment).
struct InlineItem
{
    MorphNode* node;
    float w, h;
    bool ws;
    size_t fw0 = 0, fwn = 0;
    std::string fragText;
};

// Shrink items[i] (a multi-word text run overflowing the line) to the
// longest word-prefix fitting availW, inserting the remainder as a new
// entry right after it. Boundary spaces are dropped on both sides, like
// browsers. Returns true on split; false keeps legacy atomic breaking
// (single word, fixed-width box, no renderer, empty prefix).
static bool trySplitInlineItem(std::vector<InlineItem>& items, size_t i,
                               float availW, Renderer* r)
{
    InlineItem& it = items[i];
    MorphNode* n = it.node;
    if (r == nullptr) return false;
    if (n->type != NodeType::Text && n->type != NodeType::Expr) return false;
    if (it.ws) return false;
    if (n->style.explicitWidth.isSet()) return false;
    std::string text = n->textContent();
    if (text.find('\n') != std::string::npos) return false;
    std::vector<std::string> words = splitInlineWords(text);
    size_t start = (it.fwn == 0) ? 0 : it.fw0;
    size_t total = (it.fwn == 0) ? words.size() : it.fwn;
    if (total < 2 || start + total > words.size()) return false;
    float fontSize = n->resolvedFontSize(r);
    float spaceW = r->measureTextWidth(" ", fontSize, n->style.fontWeight);
    float acc = 0.0f;
    size_t take = 0;
    for (size_t k = 0; k < total; k++)
    {
        float ww = r->measureTextWidth(words[start + k], fontSize, n->style.fontWeight);
        float add = (k == 0) ? ww : (spaceW + ww);
        if (acc + add <= availW + 0.01f) { acc += add; take = k + 1; }
        else break;
    }
    if (take == 0 || take >= total) return false;
    // Fragments each hold one visual line: single-line height, not the
    // whole node's measured height (multi-line measured text would inflate
    // every spanned row otherwise).
    float fragH = (fontSize > 0.0f) ? (fontSize * 1.4f) : it.h;
    // The boundary space belongs to the prefix box (it separates the frags
    // visually when both land on one row) while staying invisible at a line
    // end when the remainder wraps, like browsers.
    it.w = acc + spaceW;
    it.h = fragH;
    it.fw0 = start;
    it.fwn = take;
    // The boundary space rides with the prefix as a trailing blank: it
    // renders as the gap when both frags share a row, and vanishes
    // harmlessly at a line end when the remainder wraps (browsers drop
    // edge spaces the same way). The remainder never takes a leading
    // space, so a fresh line starts clean.
    it.fragText = joinInlineWords(words, start, take) + " ";
    InlineItem rest;
    rest.node = n;
    rest.h = fragH;
    rest.ws = false;
    rest.fw0 = start + take;
    rest.fwn = total - take;
    rest.fragText = joinInlineWords(words, start + take, total - take);
    rest.w = 0.0f;
    for (size_t k = 0; k < rest.fwn; k++)
    {
        float ww = r->measureTextWidth(words[rest.fw0 + k], fontSize, n->style.fontWeight);
        rest.w += (k == 0) ? ww : (spaceW + ww);
    }
    items.insert(items.begin() + (ptrdiff_t)(i + 1), std::move(rest));
    return true;
}
#endif

#ifdef MORPH_FEATURE_POSITION
// Resolve a CssLength against a base size for inset/width layout.
// Unset stays the -1e9 sentinel so existing auto checks keep working;
// every set unit resolves through the element's unit environment
// (px passes through, `%` against the base, font units against the
// element font, viewport units against the window).
inline float resolveInset(const CssLength& l, float base, const UnitEnv& env)
{
    if (!l.isSet()) return l.value;
    UnitEnv e = env;
    e.pctBase = base;
    return resolveUnits(l, e);
}

// Shift a sticky node and every descendant so children stay glued to it.
static void shiftStickySubtree(MorphNode* n, float dx, float dy) {
    n->x += dx;
    n->y += dy;
    for (auto* c : n->children) shiftStickySubtree(c, dx, dy);
}

// `position: sticky` — keeps its normal-flow box (m_flowX/m_flowY) but gets
// clamped against the nearest scroll container's scrollport, between the
// top/bottom (and left/right) offsets and its containing block. Without a
// scroll ancestor the viewport is the scrollport (page-level sticky
// headers stick on window scroll, which re-runs layout).
void MorphNode::applySticky() {
    float spLeft, spTop, spW, spH, scrolledY = 0.0f;
    MorphNode* sc = nearestScrollContainer();
    if (sc) {
#ifdef MORPH_FEATURE_BORDER
        UnitEnv scEnv = sc->unitEnv(0.0f, nullptr);
        float bw = getBorderWidth(sc->style, 0, scEnv);
#else
        float bw = 0.0f;
#endif
        spLeft = sc->x + bw + sc->m_computedPadding[3];
        spTop = sc->y + bw + sc->m_computedPadding[0];
        spW = sc->w - 2.0f * bw - sc->m_computedPadding[3] - sc->m_computedPadding[1];
        spH = sc->h - 2.0f * bw - sc->m_computedPadding[0] - sc->m_computedPadding[2];
        scrolledY = sc->scrollY;
    } else {
        spLeft = 0.0f;
        spTop = 0.0f;
        spW = m_winW;
        spH = m_winH;
    }
    if (spW < 0.0f) spW = 0.0f;
    if (spH < 0.0f) spH = 0.0f;

    float newX = m_flowX;
    float newY = m_flowY;
    // Sticky offsets belong to this element: font units use its font.
    UnitEnv env = unitEnv(0.0f, nullptr);

    if (style.left.isSet() || style.right.isSet()) {
        if (style.left.isSet()) {
            float minX = spLeft + resolveInset(style.left, spW, env);
            if (newX < minX) newX = minX;
        }
        if (style.right.isSet()) {
            float maxX = spLeft + spW - resolveInset(style.right, spW, env) - w;
            if (newX > maxX) newX = maxX;
        }
        if (parent) {
#ifdef MORPH_FEATURE_BORDER
            UnitEnv pEnv = parent->unitEnv(0.0f, nullptr);
            float pbwL = getBorderWidth(parent->style, 3, pEnv);
            float pbwR = getBorderWidth(parent->style, 1, pEnv);
#else
            float pbwL = 0.0f, pbwR = 0.0f;
#endif
            float cbLeft = parent->x + pbwL + parent->m_computedPadding[3];
            float cbW = parent->w - pbwL - pbwR - parent->m_computedPadding[3] - parent->m_computedPadding[1];
            if (cbW < 0.0f) cbW = 0.0f;
            if (newX < cbLeft) newX = cbLeft;
            float cbRight = cbLeft + cbW - w;
            if (newX > cbRight) newX = cbRight;
        }
    }

    if (style.top.isSet() || style.bottom.isSet()) {
        if (style.top.isSet()) {
            float minY = spTop + resolveInset(style.top, spH, env) + scrolledY;
            if (newY < minY) newY = minY;
        }
        if (style.bottom.isSet()) {
            float maxY = spTop + spH - resolveInset(style.bottom, spH, env) - h + scrolledY;
            if (newY > maxY) newY = maxY;
        }
        if (parent) {
#ifdef MORPH_FEATURE_BORDER
            UnitEnv pEnv = parent->unitEnv(0.0f, nullptr);
            float pbwT = getBorderWidth(parent->style, 0, pEnv);
            float pbwB = getBorderWidth(parent->style, 2, pEnv);
#else
            float pbwT = 0.0f, pbwB = 0.0f;
#endif
            float cbTop = parent->y + pbwT + parent->m_computedPadding[0];
            float cbH = parent->h - pbwT - pbwB - parent->m_computedPadding[0] - parent->m_computedPadding[2];
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
    if (style.position == CSS::Position::Sticky) applySticky();
    for (auto* c : children) c->updateStickySubtree();
}
#endif

#ifdef MORPH_FEATURE_FLEX
// CSS flexbox §9.2: a non-`auto` flex-basis sets the hypothetical main
// size. `%` resolves against the container's inner main size already in
// `env.pctBase` (against an indefinite size there is no basis);
// every other unit resolves through the item's own environment.
static bool resolveFlexBasis(const std::string& fb, const UnitEnv& env, float* basis)
{
    if (fb.empty()) return false;
    CssLength l;
    if (!parseCssLength(fb, l)) return false;
    if (l.unit == LengthUnit::Pct && env.pctBase <= 0.0f) return false;
    *basis = resolveUnits(l, env);
    return true;
}

// CSS flexbox §9.6: `align-self` overrides the container's `align-items`
// for a single item (`auto` inherits it). `baseline` has no baseline
// metrics to resolve against yet and behaves as flex-start.
static CSS::AlignItems effCrossAlign(const MorphStyle& container, const MorphStyle& item)
{
    switch (item.alignSelf)
    {
    case CSS::AlignSelf::FlexStart:
        return CSS::AlignItems::FlexStart;
    case CSS::AlignSelf::Center:
        return CSS::AlignItems::Center;
    case CSS::AlignSelf::FlexEnd:
        return CSS::AlignItems::FlexEnd;
    case CSS::AlignSelf::Stretch:
        return CSS::AlignItems::Stretch;
    case CSS::AlignSelf::Baseline:
        return CSS::AlignItems::FlexStart;
    default:
        return container.alignItems;
    }
}
#endif

void MorphNode::layout(float px, float py, float parentW, float parentH,
                       Renderer* r) {
#ifdef MORPH_FEATURE_POSITION
    // Root node: establish the viewport + initial containing block before
    // anything resolves viewport units below.
    if (!parent) {
        m_winW = parentW;
        m_winH = parentH;
        m_absCbX = 0.0f; m_absCbY = 0.0f;
        m_absCbW = parentW; m_absCbH = parentH;
    }
#endif
    // Computed font first: margins, padding, insets and sizes below may
    // all be font-relative. `%` margins/padding resolve against the
    // containing-block width the parent passed in (CSS 2.1 §8.3/§10).
    computeFontSize(r);
    UnitEnv env = unitEnv(parentW, r);
    float ml = resolveUnits(style.margin[3], env);
    float mr = resolveUnits(style.margin[1], env);
    float mt = resolveUnits(style.margin[0], env);
    float mb = resolveUnits(style.margin[2], env);
    bool autoL = style.marginAuto[3], autoR = style.marginAuto[1];
    bool autoT = style.marginAuto[0], autoB = style.marginAuto[2];

    float pl = resolveUnits(style.padding[3], env);
    float pr = resolveUnits(style.padding[1], env);
    float pt = resolveUnits(style.padding[0], env);
    float pb = resolveUnits(style.padding[2], env);
    m_computedPadding[3] = pl; m_computedPadding[1] = pr;
    m_computedPadding[0] = pt; m_computedPadding[2] = pb;
#ifdef MORPH_FEATURE_POSITION
    // A fixed node flags itself; parents flag the ancestor chain when
    // they place their fixed children (below). Resets run top-down while
    // marking flows bottom-up, so nothing is wiped after being set.
    m_subtreeHasFixed = (style.position == CSS::Position::Fixed);
#endif

#ifdef MORPH_FEATURE_BORDER
    UnitEnv bwEnv = env;
    bwEnv.pctBase = 0.0f; // border `%` is invalid CSS and never parses
    float bw = resolveUnits(style.borderWidth, bwEnv);
    float bwT = getBorderWidth(style, 0, bwEnv);
    float bwR = getBorderWidth(style, 1, bwEnv);
    float bwB = getBorderWidth(style, 2, bwEnv);
    float bwL = getBorderWidth(style, 3, bwEnv);
#else
    float bw = 0.0f;
    float bwT = 0.0f;
    float bwR = 0.0f;
    float bwB = 0.0f;
    float bwL = 0.0f;
#endif

#ifdef MORPH_FEATURE_POSITION
    bool isAbs = (style.position == CSS::Position::Absolute
                   || style.position == CSS::Position::Fixed);
    bool isRel = (style.position == CSS::Position::Relative
                   || style.position == CSS::Position::Sticky);

    // Far-edge re-resolution for out-of-flow boxes (CSS 2.1 §10.3.7/§10.6.7):
    // a right/bottom-only box with auto size is placed with a provisional
    // size here; the true size is known only after children (auto height)
    // and min/max clamps, so remember the far-edge inputs and re-resolve
    // at each growth point below.
    bool absFixRight = false, absFixBottom = false;
    float absFixCbx = 0.0f, absFixCbw = 0.0f, absFixRR = 0.0f, absFixMR = 0.0f;
    float absFixCby = 0.0f, absFixCbh = 0.0f, absFixRB = 0.0f, absFixMB = 0.0f;
    if (isAbs) {
        // ── Out of flow: absolute (nearest positioned ancestor's padding box)
        //    or fixed (viewport). px/py/parentW/parentH are ignored here.
        float cbx, cby, cbw, cbh;
        if (style.position == CSS::Position::Fixed) {
            cbx = 0.0f; cby = 0.0f;
            cbw = m_winW; cbh = m_winH;
        } else {
            cbx = m_absCbX; cby = m_absCbY;
            cbw = m_absCbW; cbh = m_absCbH;
        }

        // Every set unit resolves through the element's own environment:
        // `%` against the containing block (width for horizontal insets
        // and widths, height for vertical ones), font units against the
        // element font, viewport units against the window.
        float rL = resolveInset(style.left, cbw, env);
        float rR = resolveInset(style.right, cbw, env);
        float rT = resolveInset(style.top, cbh, env);
        float rB = resolveInset(style.bottom, cbh, env);
        bool lSet = rL > -1e8f, rSet = rR > -1e8f;
        bool tSet = rT > -1e8f, bSet = rB > -1e8f;
        float eW = resolveInset(style.explicitWidth, cbw, env);
        float eH = resolveInset(style.explicitHeight, cbh, env);
        bool wSet = eW > -1e8f, hSet = eH > -1e8f;

        // Border/padding allowance for content-box sizing.
        float padBwW = 0.0f, padBwH = 0.0f;
#ifdef MORPH_FEATURE_BORDER_BOX
        if (style.boxSizing != CSS::BoxSizing::BorderBox)
#endif
        {
            padBwW = pl + pr + bwL + bwR;
            padBwH = pt + pb + bwT + bwB;
        }

        // Width: an explicit width always wins (over-constrained keeps
        // width and ignores the far offset in LTR); left+right with auto
        // width stretches; otherwise shrink-to-fit capped by available.
        if (wSet)
            w = eW + padBwW;
        else if (lSet && rSet)
            w = cbw - rL - rR;
        else
            w = -1.0f;
        if (w < 0.0f) w = 0.0f;
        if (!wSet && !(lSet && rSet)) {
            // Auto width → shrink-to-fit (content-based), capped by available.
            float avail = cbw;
            if (lSet) avail -= rL;
            if (rSet) avail -= rR;
            float sw = r ? contentWidth(r) : 0.0f;
            w = (sw > 0.0f && sw < avail) ? sw : avail;
            if (w < 0.0f) w = 0.0f;
        }

        // Height: same rules vertically (auto height grows from in-flow
        // children through the shared auto-height path below).
        if (hSet)
            h = eH + padBwH;
        else if (tSet && bSet)
            h = cbh - rT - rB;
        else
            h = 0.0f;
        if (h < 0.0f) h = 0.0f;

        // Margins: auto absorbs leftover space (centering) only when the
        // offsets and size are all set; otherwise auto margins are 0.
        // Position: all-auto holds the static (flow) position; a set near
        // offset wins; right/bottom-only counts back from the far edge.
        // Over-constrained keeps width/height and ignores the far offset.
        float mL = ml, mR = mr, mT = mt, mB = mb;
        if (lSet && rSet) {
            // Auto margins absorb the leftover (centering); overflowing
            // space left-sticks in LTR (the used left margin is 0).
            if (autoL && autoR) {
                float share = (cbw - w - rL - rR) * 0.5f;
                if (share < 0.0f) {
                    mL = 0.0f;
                    mR = cbw - w - rL - rR;
                } else {
                    mL = mR = share;
                }
            } else if (autoL)
                mL = cbw - w - rL - rR - mr;
            else if (autoR)
                mR = cbw - w - rL - rR - ml;
            x = cbx + rL + mL;
        } else {
            if (autoL) mL = 0.0f;
            if (autoR) mR = 0.0f;
            if (!lSet && !rSet)
                x = m_staticX + mL;
            else if (lSet)
                x = cbx + rL + mL;
            else
                x = cbx + cbw - rR - w - mR;
            if (!lSet && rSet) {
                absFixRight = true;
                absFixCbx = cbx; absFixCbw = cbw;
                absFixRR = rR; absFixMR = mR;
            }
        }
        if (tSet && bSet) {
            if (autoT && autoB) {
                float share = (cbh - h - rT - rB) * 0.5f;
                if (share < 0.0f) {
                    mT = 0.0f;
                    mB = cbh - h - rT - rB;
                } else {
                    mT = mB = share;
                }
            } else if (autoT)
                mT = cbh - h - rT - rB - mb;
            else if (autoB)
                mB = cbh - h - rT - rB - mt;
            y = cby + rT + mT;
        } else {
            if (autoT) mT = 0.0f;
            if (autoB) mB = 0.0f;
            if (!tSet && !bSet)
                y = m_staticY + mT;
            else if (tSet)
                y = cby + rT + mT;
            else
                y = cby + cbh - rB - h - mB;
            if (!tSet && bSet) {
                absFixBottom = true;
                absFixCby = cby; absFixCbh = cbh;
                absFixRB = rB; absFixMB = mB;
            }
        }
        m_computedMargin[3] = mL; m_computedMargin[1] = mR;
        m_computedMargin[0] = mT; m_computedMargin[2] = mB;
    } else
#endif
    {
    float mlForWidth = autoL ? 0.0f : ml;
    float mrForWidth = autoR ? 0.0f : mr;

    if (style.explicitWidth.isSet()) {
        // `%` against the containing-block width the parent passed in
        // (same base as the auto fill below); every other unit resolves
        // through the element's own environment.
        float ew = resolveUnits(style.explicitWidth, env);
#ifdef MORPH_FEATURE_BORDER_BOX
        if (style.boxSizing == CSS::BoxSizing::BorderBox) {
            w = ew;
        } else
#endif
        {
            w = ew + pl + pr + bwL + bwR;
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
               ::toString(type), px, py, mt, mb, py + mt);
    }
    m_computedMargin[3] = ml; m_computedMargin[1] = mr;
    m_computedMargin[0] = mt; m_computedMargin[2] = mb;

    x = px + ml;
    y = py + mt;

    // `%` heights resolve against the containing-block height when
    // definite; against an indefinite (auto) height they behave as
    // auto, per spec. Every other unit resolves unconditionally.
    if (style.explicitHeight.isSet() && (!style.explicitHeight.isPercent() || parentH > 0.0f)) {
        UnitEnv hEnv = env;
        hEnv.pctBase = parentH;
        float eh = resolveUnits(style.explicitHeight, hEnv);
#ifdef MORPH_FEATURE_BORDER_BOX
        if (style.boxSizing == CSS::BoxSizing::BorderBox) {
            h = eh;
        } else
#endif
        {
            h = eh + pt + pb + bwT + bwB;
        }
    } else {
        h = 0.0f;
    }

#ifdef MORPH_FEATURE_POSITION
    // ── Relative: offset the flow box without affecting siblings. ──
    // Sticky skips the fixed offset here — its offset is the scroll clamp
    // applied in applySticky() (called below), anchored at m_flowX/m_flowY.
    if (style.position == CSS::Position::Relative) {
        // `%` against the containing-block size the parent passed in:
        // width for left/right, height for top/bottom (against an
        // indefinite height they evaluate to 0, i.e. auto). Font and
        // viewport units resolve through the element's environment.
        float offX = 0.0f, offY = 0.0f;
        if (style.left.isSet()) offX = resolveInset(style.left, parentW, env);
        else if (style.right.isSet()) offX = -resolveInset(style.right, parentW, env);
        if (style.top.isSet()) offY = resolveInset(style.top, parentH, env);
        else if (style.bottom.isSet()) offY = -resolveInset(style.bottom, parentH, env);
        x += offX;
        y += offY;
        m_relOffX = offX;
        m_relOffY = offY;
    }
#endif
    }

#ifdef MORPH_FEATURE_POSITION
    m_flowX = x;
    m_flowY = y;
#endif

#ifdef MORPH_FEATURE_MIN_MAX
    // `%` clamps resolve against the containing block (width for widths,
    // height for heights when definite); other units via the environment.
    if (style.minWidth.isSet()) {
        float v = resolveUnits(style.minWidth, env);
        if (v > 0.0f && w < v) w = v;
    }
    if (style.maxWidth.isSet()) {
        float v = resolveUnits(style.maxWidth, env);
        if (v > 0.0f && w > v) w = v;
    }
    if (style.minHeight.isSet() && (!style.minHeight.isPercent() || parentH > 0.0f)) {
        UnitEnv hEnv = env;
        hEnv.pctBase = parentH;
        float v = resolveUnits(style.minHeight, hEnv);
        if (v > 0.0f && h < v) h = v;
    }
    if (style.maxHeight.isSet() && (!style.maxHeight.isPercent() || parentH > 0.0f)) {
        UnitEnv hEnv = env;
        hEnv.pctBase = parentH;
        float v = resolveUnits(style.maxHeight, hEnv);
        if (v > 0.0f && h > v) h = v;
    }
#endif
#ifdef MORPH_FEATURE_POSITION
    // Right-only boxes count back from the far edge, so a min/max width
    // clamp above must move the box (children are not laid out yet, so no
    // subtree shift is needed here).
    if (absFixRight) {
        x = absFixCbx + absFixCbw - absFixRR - w - absFixMR;
        m_flowX = x;
    }
#endif

    float cw = w - pl - pr - bwL - bwR;
    if (cw < 0.0f) cw = 0.0f;
    float ch = h - pt - pb - bwT - bwB;
    if (ch < 0.0f) ch = 0.0f;
    float cx = x + bwL + pl;
    float cy = y + bwT + pt;

#ifdef MORPH_FEATURE_POSITION
    // Containing block for absolute descendants = padding box of the nearest
    // positioned ancestor (this node if positioned, otherwise inherited).
    float cbX, cbY, cbW, cbH;
    if (isPositioned()) {
        cbX = x + bwL; cbY = y + bwT;
        cbW = w - bwL - bwR; cbH = h - bwT - bwB;
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
    if (style.display == CSS::Display::None) {
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
#ifdef MORPH_FEATURE_POSITION
    // Sticky boxes lay out in flow like their static siblings; their
    // scroll clamps resolve after our height is final (see below), once
    // every box the clamp reads — including the parent box — has real
    // geometry.
    std::vector<MorphNode*> stickyChildren;
#endif
    for (auto* c : children) {
#ifdef MORPH_FEATURE_POSITION
        if (c->style.position == CSS::Position::Absolute) {
            absChildren.push_back(c);
            continue;
        }
        if (c->style.position == CSS::Position::Fixed) {
            fixedChildren.push_back(c);
            continue;
        }
        if (c->style.position == CSS::Position::Sticky)
            stickyChildren.push_back(c);
#endif
#ifdef MORPH_FEATURE_DISPLAY_NONE
        if (c->style.display == CSS::Display::None) {
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
    bool isRow = (style.display == CSS::Display::Flex
                  && (style.flexDirection == CSS::FlexDirection::Row
                      || style.flexDirection == CSS::FlexDirection::RowReverse));
    bool isCol = !isRow;
    // CSS flexbox §5: row-reverse / column-reverse flow the main axis
    // opposite (main-start = right / bottom). Line breaking stays in DOM
    // order; only the order within each line is reversed (see below).
    bool isReverse = (style.display == CSS::Display::Flex
                      && (style.flexDirection == CSS::FlexDirection::RowReverse
                          || style.flexDirection == CSS::FlexDirection::ColumnReverse));
#else
    bool isRow = false;
    bool isCol = true;
#endif
    int count = (int)normal.size();

#ifdef MORPH_FEATURE_FLEX
    if (style.display == CSS::Display::Flex) {
        struct FlexItem { MorphNode* node; float main, cross, mt, mr, mb, ml; bool mtAuto, mbAuto, mlAuto, mrAuto; };
        std::vector<FlexItem> items;

        for (auto* c : normal) {
            if (c->isWhitespaceOnly()) continue;
            c->layout(0.0f, 0.0f, cw, 0.0f, r);

            if (isRow && !c->style.explicitWidth.isSet()) {
                float cwVal = c->contentWidth(r);
                if (cwVal > 0.0f) c->w = cwVal;
            }

            // CSS flexbox §9.2: a non-`auto` flex-basis sets the hypothetical
            // main size, overriding the main-size property for flexing.
            // Percentages resolve against the container's inner main size;
            // against an indefinite size (e.g. auto-height column) they fall
            // back to the content size above. `flex: 1` relies on this: its
            // `0%` basis makes items share space equally from zero, not from
            // content width.
            if (c->style.flexBasis != "auto") {
                const std::string& fb = c->style.flexBasis;
                // `%` against the container's inner main size (indefinite
                // sizes fall back to content, as before); every other unit
                // resolves through the item's own environment.
                UnitEnv fbEnv = c->unitEnv(isCol ? ch : cw, r);
                float basis = 0.0f;
                bool haveBasis = resolveFlexBasis(fb, fbEnv, &basis);
                if (haveBasis) {
                    if (basis < 0.0f) basis = 0.0f;
                    if (isRow) {
                        c->w = basis + hBonus(c, cw, r);
                    } else {
                        c->h = basis + vBonus(c, ch, r);
                    }
                }
            }

            UnitEnv cEnv = c->unitEnv(cw, r);
            float cmt = resolveUnits(c->style.margin[0], cEnv);
            float cmb = resolveUnits(c->style.margin[2], cEnv);
            float cml = resolveUnits(c->style.margin[3], cEnv);
            float cmr = resolveUnits(c->style.margin[1], cEnv);
            bool cmtA = c->style.marginAuto[0], cmbA = c->style.marginAuto[2];
            bool cmlA = c->style.marginAuto[3], cmrA = c->style.marginAuto[1];
            float childMain = isCol ? (c->h + cmt + cmb) : (c->w + cml + cmr);
            float childCross = isCol ? (c->w + cml + cmr) : (c->h + cmt + cmb);
            items.push_back({c, childMain, childCross, cmt, cmr, cmb, cml, cmtA, cmbA, cmlA, cmrA});
        }

        float mainAvail = isCol ? ch : cw;
        // `gap` percentages resolve against the content box on their own
        // axis: main gaps against the inner main size, cross gaps against
        // the inner cross size. Font/viewport units use the container font.
        UnitEnv gapMainEnv = env;
        gapMainEnv.pctBase = isCol ? ch : cw;
        UnitEnv gapCrossEnv = env;
        gapCrossEnv.pctBase = isCol ? cw : ch;
        float gapMain = resolveUnits(style.gap, gapMainEnv);
        float gapCross = resolveUnits(style.gap, gapCrossEnv);
        // CSS flexbox §8.3: `wrap` and `wrap-reverse` both break lines;
        // reverse only flips the cross stacking direction (handled below).
        bool doWrap = (style.flexWrap == CSS::FlexWrap::Wrap
                       || style.flexWrap == CSS::FlexWrap::WrapReverse);
        bool wrapReverse = (style.flexWrap == CSS::FlexWrap::WrapReverse);

        struct FlexLine { std::vector<FlexItem*> fItems; float crossSize = 0.0f; float totalMain = 0.0f; };
        std::vector<FlexLine> lines;
        FlexLine curLine;

        for (auto& item : items) {
            if (doWrap && !curLine.fItems.empty()
                && curLine.totalMain + gapMain + item.main > mainAvail) {
                lines.push_back(curLine);
                curLine = FlexLine();
            }
            curLine.fItems.push_back(&item);
            curLine.totalMain += item.main + (curLine.fItems.size() > 1 ? gapMain : 0.0f);
            if (item.cross > curLine.crossSize) curLine.crossSize = item.cross;
        }
        if (!curLine.fItems.empty()) lines.push_back(curLine);
        if (lines.empty()) lines.push_back(FlexLine());

        // CSS flexbox §5.3: *-reverse keeps DOM line breaking but lays each
        // line's items in reverse order. Reversing per-line (not globally)
        // keeps wrap lines correct: Line1 still holds the first DOM items,
        // just positioned main-end-first.
        if (isReverse) {
            for (auto& line : lines) {
                std::reverse(line.fItems.begin(), line.fItems.end());
            }
        }
        // In a reversed axis main-start is the opposite end, so pack
        // flex-start where forward would pack flex-end and vice versa.
        // Center / space-* are symmetric under mirroring and stay as-is.
        CSS::JustifyContent effJustify = style.justifyContent;
        if (isReverse) {
            if (effJustify == CSS::JustifyContent::FlexStart) {
                effJustify = CSS::JustifyContent::FlexEnd;
            } else if (effJustify == CSS::JustifyContent::FlexEnd) {
                effJustify = CSS::JustifyContent::FlexStart;
            }
        }

        for (auto& line : lines) {
            float extraGap = line.fItems.size() > 1 ? gapMain * (line.fItems.size() - 1) : 0.0f;
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

            // Grow/shrink run after the child's own layout pass, so
            // re-apply its min/max clamp on the main axis (which that
            // pass already enforced before flex touched the size).
#ifdef MORPH_FEATURE_MIN_MAX
            for (auto* item : line.fItems) {
                auto& st = item->node->style;
                // Item's own environment: `%` clamps against the container
                // inner main size, font units against the item font.
                UnitEnv itemEnv = item->node->unitEnv(isRow ? cw : ch, r);
                auto clampW = [&](const CssLength& lim) -> float {
                    return resolveUnits(lim, itemEnv);
                };
                if (isRow) {
                    if (st.minWidth.isSet()) {
                        float v = clampW(st.minWidth);
                        if (v > 0.0f && item->node->w < v) {
                            item->main += v - item->node->w;
                            item->node->w = v;
                        }
                    }
                    if (st.maxWidth.isSet()) {
                        float v = clampW(st.maxWidth);
                        if (v > 0.0f && item->node->w > v) {
                            item->main -= item->node->w - v;
                            item->node->w = v;
                        }
                    }
                    if (item->node->w < 0.0f) {
                        item->main -= item->node->w;
                        item->node->w = 0.0f;
                    }
                } else {
                    // Column main axis: `%` heights need a definite cross
                    // size, else the clamp is skipped like the item's own
                    // height would be.
                    UnitEnv hItemEnv = item->node->unitEnv(ch, r);
                    auto clampH = [&](const CssLength& lim) -> float {
                        return resolveUnits(lim, hItemEnv);
                    };
                    bool hDefinite = ch > 0.0f;
                    if (st.minHeight.isSet() && (!st.minHeight.isPercent() || hDefinite)) {
                        float v = clampH(st.minHeight);
                        if (v > 0.0f && item->node->h < v) {
                            item->main += v - item->node->h;
                            item->node->h = v;
                        }
                    }
                    if (st.maxHeight.isSet() && (!st.maxHeight.isPercent() || hDefinite)) {
                        float v = clampH(st.maxHeight);
                        if (v > 0.0f && item->node->h > v) {
                            item->main -= item->node->h - v;
                            item->node->h = v;
                        }
                    }
                    if (item->node->h < 0.0f) {
                        item->main -= item->node->h;
                        item->node->h = 0.0f;
                    }
                }
            }
#else
            for (auto* item : line.fItems) {
                if (isRow && item->node->w < 0.0f) {
                    item->main -= item->node->w;
                    item->node->w = 0.0f;
                }
                if (!isRow && item->node->h < 0.0f) {
                    item->main -= item->node->h;
                    item->node->h = 0.0f;
                }
            }
#endif
        }

        float mainStart = isCol ? cy : cx;
        float crossStart = isCol ? cx : cy;
        float crossSize = isCol ? cw : ch;
        float cursorCross = crossStart;

        // CSS flexbox §8.3 `wrap-reverse`: lines stack from cross-end to
        // cross-start (row: bottom-up, column: right-to-left). Precompute
        // each line's cross origin working back from the cross end so the
        // first DOM line lands at the end. With a definite cross size the
        // stack packs to the end (extra space at the start); with auto
        // cross the stack exactly fills the grown box. Single-line still
        // applies: it sits at the end when definite, at the start when
        // auto (identical either way).
        std::vector<float> revBase;
        if (wrapReverse && !lines.empty()) {
            float totalCross = 0.0f;
            for (auto& ln : lines) {
                totalCross += ln.crossSize;
            }
            if (lines.size() > 1) {
                totalCross += gapCross * (float)(lines.size() - 1);
            }
            float crossExtent = totalCross;
            if (isCol) {
                if (cw > crossExtent) {
                    crossExtent = cw;
                }
            } else if (style.explicitHeight.isSet() && ch > crossExtent) {
                crossExtent = ch;
            }
            revBase.resize(lines.size());
            float cc = crossStart + crossExtent;
            for (size_t li = 0; li < lines.size(); li++) {
                cc -= lines[li].crossSize;
                revBase[li] = cc;
                cc -= gapCross;
            }
        }

        for (size_t li = 0; li < lines.size(); li++) {
            auto& line = lines[li];
            float lineBaseCross = (wrapReverse && li < revBase.size()) ? revBase[li] : cursorCross;
            float lineCross = line.crossSize;
            // A single-line flex container with a definite cross size
            // stretches its line to fill it (CSS flexbox §9.7): otherwise
            // `align-items: stretch` has nothing to stretch
            // intrinsically-empty items to (e.g. flex-grown cells with no
            // content of their own collapse to zero on the cross axis).
            if (!doWrap) {
                float definiteCross = -1.0f;
                UnitEnv dcEnv = env;
                if (isCol && style.explicitWidth.isSet()) {
                    dcEnv.pctBase = cw;
                    definiteCross = style.explicitWidth.isPercent()
                        ? resolveUnits(style.explicitWidth, dcEnv)
                        : cw;
                }
                if (!isCol && style.explicitHeight.isSet()) {
                    dcEnv.pctBase = ch;
                    definiteCross = style.explicitHeight.isPercent()
                        ? resolveUnits(style.explicitHeight, dcEnv)
                        : ch;
                }
                if (definiteCross > lineCross) lineCross = definiteCross;
            }
            float extraGap = line.fItems.size() > 1 ? gapMain * (line.fItems.size() - 1) : 0.0f;
            float free = mainAvail - line.totalMain;

            // Distribute free space to auto margins on main axis
            std::vector<FlexItem*> autoMainItems;
            for (auto* item : line.fItems) {
                if (isCol) {
                    if (item->mtAuto || item->mbAuto) autoMainItems.push_back(item);
                } else {
                    if (item->mlAuto || item->mrAuto) autoMainItems.push_back(item);
                }
            }
            if (!autoMainItems.empty() && free > 0.0f) {
                float perAuto = free / autoMainItems.size();
                for (auto* item : autoMainItems) {
                    if (isCol) {
                        if (item->mtAuto) item->mt += perAuto;
                        else if (item->mbAuto) item->mb += perAuto;
                    } else {
                        if (item->mlAuto) item->ml += perAuto;
                        else if (item->mrAuto) item->mr += perAuto;
                    }
                    item->main += perAuto;
                }
                // Recompute free after auto margin absorption
                // (totalMain already held the gaps; keep them out).
                float newTotal = extraGap;
                for (auto* item : line.fItems) newTotal += item->main;
                free = mainAvail - newTotal;
            }

            float offset = 0.0f;
            float itemGap = gapMain;
            if (effJustify == CSS::JustifyContent::Center) {
                offset = free * 0.5f;
            } else if (effJustify == CSS::JustifyContent::FlexEnd) {
                offset = free;
            } else if (effJustify == CSS::JustifyContent::SpaceBetween) {
                offset = 0.0f;
                itemGap = (line.fItems.size() > 1) ? gapMain + free / (line.fItems.size() - 1) : 0.0f;
            } else if (effJustify == CSS::JustifyContent::SpaceAround) {
                offset = line.fItems.size() > 0 ? free / (line.fItems.size() * 2) : 0.0f;
                itemGap = line.fItems.size() > 0 ? gapMain + free / line.fItems.size() : 0.0f;
            }

            float cursor = mainStart + offset;

            for (size_t i = 0; i < line.fItems.size(); i++) {
                auto* ci = line.fItems[i];
                float childMain = isCol ? ci->node->h : ci->node->w;
                float crossDim  = isCol ? ci->node->w : ci->node->h;
                CSS::AlignItems effAlign = effCrossAlign(style, ci->node->style);

                float posMain = cursor + (isCol ? ci->mt : ci->ml);
                float posCross = lineBaseCross + (isCol ? ci->ml : ci->mt);

                if (lineCross > crossDim) {
                    if (effAlign == CSS::AlignItems::Center) {
                        float marginCross = isCol ? (ci->ml + ci->mr) : (ci->mt + ci->mb);
                        posCross = lineBaseCross + (isCol ? ci->ml : ci->mt) + (lineCross - (crossDim + marginCross)) * 0.5f;
                    } else if (effAlign == CSS::AlignItems::FlexEnd) {
                        posCross = lineBaseCross + lineCross - crossDim;
                        posCross -= (isCol ? ci->mr : ci->mb);
                    }
                }

                float childX = isCol ? posCross : posMain;
                float childY = isCol ? posMain : posCross;
                float childPW = isCol ? ((effAlign == CSS::AlignItems::Stretch) ? crossSize : crossDim) : childMain;
                float childPH = isCol ? childMain : ((effAlign == CSS::AlignItems::Stretch) ? lineCross : crossDim);

                if (effAlign != CSS::AlignItems::Stretch && !ci->node->style.explicitWidth.isSet() && isCol) {
                    float cwVal = ci->node->contentWidth(r);
                    if (cwVal > 0.0f && cwVal < childPW) {
                        crossDim = cwVal;
                        childPW = cwVal;
                        if (lineCross > crossDim) {
                            if (effAlign == CSS::AlignItems::Center)
                                posCross = lineBaseCross + (lineCross - crossDim) * 0.5f;
                            else if (effAlign == CSS::AlignItems::FlexEnd)
                                posCross = lineBaseCross + lineCross - crossDim;
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

                if (effAlign == CSS::AlignItems::Stretch && !ci->node->style.explicitWidth.isSet() && isCol) {
                    float availW = lineCross - ci->ml - ci->mr;
                    if (availW < 0.0f) availW = 0.0f;
                    if (availW > ci->node->w) ci->node->w = availW;
                }
                if (effAlign == CSS::AlignItems::Stretch && !ci->node->style.explicitHeight.isSet() && isRow) {
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

            cursorCross += lineCross + gapCross;
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
        // Transparent containers spliced into the run (links, spans):
        // boxes rebuilt from children at flush end. Cleared with the run.
        std::vector<MorphNode*> unwrappedInline;

        auto flushInline = [&]() {
            if (currentInline.empty()) return;

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
            if (!firstBlockChild) inlineBeforeFirstBlock = true;
            else inlineAfterLastBlock = true;
#endif
            std::vector<InlineItem> items;
            for (auto* c : currentInline) {
                if (c->type == NodeType::Text || c->type == NodeType::Expr)
                    c->m_frags.clear();
                c->layout(0.0f, 0.0f, cw, 0.0f, r);
                float iw = 0.0f;
                if (c->style.explicitWidth.isSet()) {
                    iw = c->w;
                } else if (c->contentWidth(r) > 0.0f) {
                    iw = c->contentWidth(r);
                }
                // Empty text runs (e.g. an expression before its effect
                // delivers content) measure zero — never a full line, which
                // would push siblings down a line until content arrives.
                if (iw <= 0.0f)
                    iw = c->isEmptyText() ? 0.0f : cw;
                float ih = (c->h > 0.0f) ? c->h : (c->resolvedFontSize(r) * 1.4f);
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
                unwrappedInline.clear();
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
                    items[i].w = r ? r->measureTextWidth(" ", items[i].node->resolvedFontSize(r), items[i].node->style.fontWeight) : 4.0f;
                    items[i].h = 0.0f;
                }
            }
            // Runs sharing a line share one baseline: more than one visible
            // text run disables per-run optical centering (each run's own
            // ink box would put siblings on slightly different baselines).
            // Solo runs keep optical centering. Reset every pass since
            // group membership changes with content.
            int textRuns = 0;
            for (size_t i = (size_t)firstVis; i <= (size_t)lastVis; i++) {
                if (!items[i].ws && items[i].node->isTextRun()) {
                    textRuns++;
                }
            }
            for (auto* c : currentInline) {
                c->m_centerInk = (textRuns <= 1);
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
                if (lineStart > 0 || style.textAlign == CSS::TextAlign::Center || style.textAlign == CSS::TextAlign::Right) {
                    if (style.textAlign == CSS::TextAlign::Center)
                        alignX = cx + (cw - lineW) * 0.5f;
                    else if (style.textAlign == CSS::TextAlign::Right)
                        alignX = cx + cw - lineW;
                }
                float itemX = alignX;
                for (size_t j = lineStart; j < end; j++) {
                    auto& p = items[j];
                    // Continuation fragments carry no margins (accounted on
                    // the run's first fragment).
                    bool cont = (p.fwn != 0 && p.fw0 != 0);
                    UnitEnv pEnv = p.node->unitEnv(cw, r);
                    float pml = cont ? 0.0f : resolveUnits(p.node->style.margin[3], pEnv);
                    float pmr = cont ? 0.0f : resolveUnits(p.node->style.margin[1], pEnv);
                    // Child-to-parent: the item's box comes from its own
                    // measured size; children keep whatever THEIR layout
                    // pass computed, translated by the item's move delta.
                    // Siblings are never restacked and widths are never
                    // overwritten here — every child preserves its laid-out
                    // offset relative to the item (inline runs stay
                    // side-by-side, blocks stay stacked).
                    float prevX = p.node->x, prevY = p.node->y;
                    float prevW = p.node->w, prevH = p.node->h;
                    p.node->x = itemX + pml;
                    p.node->y = lineY;
                    p.node->w = (p.w < cw) ? p.w : cw;
                    p.node->h = p.h;
                    float dx = p.node->x - prevX;
                    float dy = p.node->y - prevY;
                    bool resized = (p.node->w != prevW || p.node->h != prevH);
                    if (dx != 0.0f || dy != 0.0f)
                    {
                        shiftChildrenSubtree(p.node, dx, dy);
                        markSubtreePaintDirty(p.node);
                    }
                    if (resized && p.fwn == 0)
                    {
                        // The item's size changed after measure: re-run its
                        // OWN layout at the final box so content resolves
                        // against real dimensions (centering, wrapping).
                        // Measure-time positions (e.g. text centered in the
                        // full container width) would otherwise stick, since
                        // the item is skipped as clean afterwards. Each
                        // child is positioned by the item's own flow logic —
                        // never restacked at one shared origin.
                        // (Skipped for word fragments: their geometry comes
                        // from line breaking, and re-laying the whole node
                        // at a fragment box would clobber sibling frags.)
                        // Relative items re-layout from the unoffset box:
                        // passing the placed box would add their offset
                        // twice (same hazard as layoutIfNeeded above).
                        float rpx = p.node->x, rpy = p.node->y;
#ifdef MORPH_FEATURE_POSITION
                        if (p.node->style.position == CSS::Position::Relative) {
                            rpx -= p.node->m_relOffX;
                            rpy -= p.node->m_relOffY;
                        }
#endif
                        p.node->layout(rpx, rpy,
                                       p.node->w, p.node->h, r);
                        markSubtreePaintDirty(p.node);
                    }
                    // Word fragments paint from their own slice: record the
                    // box + text now (the node box below is shared across
                    // fragments and gets unioned after the run).
                    if (p.fwn != 0)
                    {
                        MorphNode::TextFrag frag;
                        frag.text = p.fragText;
                        frag.x = p.node->x;
                        frag.y = p.node->y;
                        frag.w = p.node->w;
                        frag.h = p.node->h;
                        p.node->m_frags.push_back(std::move(frag));
                    }
                    // NOTE: children are never assigned here directly. The
                    // item's own passes own their geometry (child-to-parent);
                    // re-laying siblings at one shared origin is what merged
                    // inline runs (the +/- overlap). An oversized item's
                    // content visibly overflows, like a browser, instead of
                    // merging.
                    itemX += pml + p.w + pmr;
                }
            };

            for (size_t i = 0; i < items.size(); i++) {
                // Continuation fragments (word range past the first) carry
                // no margins of their own — the run's margins were already
                // accounted on its first fragment.
                auto isCont = [&](size_t k) -> bool {
                    return items[k].fwn != 0 && items[k].fw0 != 0;
                };
                UnitEnv iEnv = items[i].node->unitEnv(cw, r);
                float ml = isCont(i) ? 0.0f : resolveUnits(items[i].node->style.margin[3], iEnv);
                float mr = isCont(i) ? 0.0f : resolveUnits(items[i].node->style.margin[1], iEnv);
                float need = ml + items[i].w + mr;

                if (i > lineStart && lineX + need > cx + cw) {
                    // Overflow with content on the line: try splitting a
                    // multi-word text run at word boundaries (browser line
                    // breaking). On success items[i] shrinks to the fitting
                    // prefix and the remainder is inserted right after it,
                    // so it flows onto the following lines.
                    if (trySplitInlineItem(items, i, (cx + cw) - lineX - ml - mr, r)) {
                        ml = isCont(i) ? 0.0f : resolveUnits(items[i].node->style.margin[3], iEnv);
                        mr = isCont(i) ? 0.0f : resolveUnits(items[i].node->style.margin[1], iEnv);
                        need = ml + items[i].w + mr;
                    } else {
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
                }

                lineX += need;
                if (items[i].h > lineH) lineH = items[i].h;
            }

            positionItems(items.size());
            float groupBottom = lineY + lineH;
            if (groupBottom > maxBottom) maxBottom = groupBottom;
            curY = groupBottom;
#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
            prevMb = 0.0f;
#endif
            // Fragmented runs span lines: restore each fragmented node's
            // box to the union of its fragment boxes (hit-testing,
            // centering, clipping). Unfragmented nodes are untouched.
            for (auto* c : currentInline) {
                if (c->m_frags.empty()) continue;
                float ux0 = c->m_frags[0].x, uy0 = c->m_frags[0].y;
                float ux1 = ux0 + c->m_frags[0].w, uy1 = uy0 + c->m_frags[0].h;
                for (auto& f : c->m_frags) {
                    if (f.x < ux0) ux0 = f.x;
                    if (f.y < uy0) uy0 = f.y;
                    if (f.x + f.w > ux1) ux1 = f.x + f.w;
                    if (f.y + f.h > uy1) uy1 = f.y + f.h;
                }
                c->x = ux0; c->y = uy0;
                c->w = ux1 - ux0; c->h = uy1 - uy0;
            }
            // Spliced transparent containers (links, spans): rebuild each
            // box from its children, innermost first, so a later real
            // layout of the container repositions children identically
            // instead of clobbering run positions with stale ones.
            for (auto it = unwrappedInline.rbegin(); it != unwrappedInline.rend(); ++it) {
                MorphNode* n = *it;
                bool any = false;
                float ux0 = 0, uy0 = 0, ux1 = 0, uy1 = 0;
                for (auto* c : n->children) {
                    if (!any) {
                        ux0 = c->x; uy0 = c->y;
                        ux1 = c->x + c->w; uy1 = c->y + c->h;
                        any = true;
                    } else {
                        if (c->x < ux0) ux0 = c->x;
                        if (c->y < uy0) uy0 = c->y;
                        if (c->x + c->w > ux1) ux1 = c->x + c->w;
                        if (c->y + c->h > uy1) uy1 = c->y + c->h;
                    }
                }
                if (any) {
                    n->x = ux0; n->y = uy0;
                    n->w = ux1 - ux0; n->h = uy1 - uy0;
                    markSubtreePaintDirty(n);
                    // Stand the container down for this pass: the run
                    // already resolved the whole subtree, so a later real
                    // layout of the container would only clobber run
                    // positions with stale-box ones (link labels landing
                    // rows away from their boxes). Future dirt re-arms it.
                    n->clearDirty(LayoutDirty);
                    n->clearDirty(StyleDirty);
                    n->clearDirty(SubtreeDirty);
                }
            }
            currentInline.clear();
            unwrappedInline.clear();
        };

        for (auto* c : normal) {
            if (c->style.display == CSS::Display::Inline
                || c->style.display == CSS::Display::InlineBlock
                || c->type == NodeType::Text || c->type == NodeType::Expr) {
                pushInlineRunNode(currentInline, c, &unwrappedInline);
            } else {
                flushInline();

                UnitEnv cEnv = c->unitEnv(cw, r);
                float ownMt = resolveUnits(c->style.margin[0], cEnv);
                float ownMb = resolveUnits(c->style.margin[2], cEnv);

#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
                if (getenv("MORPH_LAYOUT_DEBUG")) {
                    printf("[layout] %s child type=%s y=%.2f curY=%.2f cy=%.2f ownMt=%.2f firstBlock=%d inlineBefore=%d pt=%g bw=%g\n",
                           CSS::toString(style.display), ::toString(c->type), c->y, curY, cy, ownMt,
                           firstBlockChild ? 1 : 0, inlineBeforeFirstBlock ? 1 : 0, pt, bw);
                }
                // A child's collapsed-through margins (m_computedMargin) are
                // only known after its own layout pass, so lay it out once at
                // a provisional y to learn them, then move it to its final y
                // and relayout only if the position changed.  This keeps the
                // very first layout pass correct (no stale-margin pass 1).
                float provY = (!firstBlockChild && !inlineBeforeFirstBlock
                               && pt == 0.0f && bwT == 0.0f)
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
                    && pt == 0.0f && bwT == 0.0f)
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
            UnitEnv cEnv = c->unitEnv(cw, r);
            float ownMt = resolveUnits(c->style.margin[0], cEnv);
            float ownMb = resolveUnits(c->style.margin[2], cEnv);

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
    // affect this node's height. Each records its static (flow-slot)
    // position first: the content-box origin, or the bottom margin edge of
    // the last preceding in-flow block sibling. Inline runs are skipped
    // (line-box accounting would be needed for exactness there).
    for (auto* c : absChildren) {
        float sx = cx, sy = cy;
        for (auto* s : children) {
            if (s == c) break;
            if (s->style.display == CSS::Display::None) continue;
            if (s->style.position == CSS::Position::Absolute
                || s->style.position == CSS::Position::Fixed)
                continue;
            if (s->isWhitespaceOnly()) continue;
            if (s->style.display == CSS::Display::Inline
                || s->type == NodeType::Text || s->type == NodeType::Expr)
                continue;
            sy = s->y + s->h + s->m_computedMargin[2];
        }
        c->m_staticX = sx;
        c->m_staticY = sy;
        c->layout(0.0f, 0.0f, 0.0f, 0.0f, r);
    }
    // Fixed children are positioned relative to the viewport.
    for (auto* c : fixedChildren) {
        for (MorphNode* p = this; p; p = p->parent)
            p->m_subtreeHasFixed = true;
        float sx = cx, sy = cy;
        for (auto* s : children) {
            if (s == c) break;
            if (s->style.display == CSS::Display::None) continue;
            if (s->style.position == CSS::Position::Absolute
                || s->style.position == CSS::Position::Fixed)
                continue;
            if (s->isWhitespaceOnly()) continue;
            if (s->style.display == CSS::Display::Inline
                || s->type == NodeType::Text || s->type == NodeType::Expr)
                continue;
            sy = s->y + s->h + s->m_computedMargin[2];
        }
        c->m_staticX = sx;
        c->m_staticY = sy;
        c->layout(0.0f, 0.0f, 0.0f, 0.0f, r);
    }
#endif

    if (!style.explicitHeight.isSet()) {
        float autoH = (maxBottom - cy) + pt + pb + bwT + bwB;
#ifdef MORPH_FEATURE_MARGIN_COLLAPSE
        // Parent–child margin collapse: the last block child's bottom margin
        // collapses through a boundary-less parent, so it must not inflate
        // our height — our parent applies it as the gap after us instead.
        if (pb == 0.0f && bwB == 0.0f && lastBlockChildMbSet && !inlineAfterLastBlock)
            autoH -= lastChildMbEff;
#endif
        if (autoH < 0.0f) autoH = 0.0f;
        if (autoH > h) h = autoH;
    }

#ifdef MORPH_FEATURE_FLEX
    if (style.display == CSS::Display::Flex && !style.explicitWidth.isSet() && isRow
        && maxRight > cx + cw) {
        float autoW = maxRight - x + pr + bwR;
        if (autoW > w) w = autoW;
    }
#endif

#ifdef MORPH_FEATURE_MIN_MAX
    if (style.minHeight.isSet() && (!style.minHeight.isPercent() || parentH > 0.0f)) {
        UnitEnv hEnv = env;
        hEnv.pctBase = parentH;
        float v = resolveUnits(style.minHeight, hEnv);
        if (v > 0.0f && h < v) h = v;
    }
    if (style.maxHeight.isSet() && (!style.maxHeight.isPercent() || parentH > 0.0f)) {
        UnitEnv hEnv = env;
        hEnv.pctBase = parentH;
        float v = resolveUnits(style.maxHeight, hEnv);
        if (v > 0.0f && h > v) h = v;
    }
#endif

    if (!style.explicitHeight.isSet() &&
        (style.overflow == CSS::Overflow::Auto || style.overflow == CSS::Overflow::Scroll) &&
        parentH > 0.0f && h > parentH) {
        h = parentH;
    }

#ifdef MORPH_FEATURE_POSITION
    // Far-edge boxes count back against their final size (CSS 2.1
    // §10.3.7/§10.6.7): the initial placement above used a provisional
    // size, and min/max clamps plus auto-height growth above may have
    // changed it since. Children were laid out at the provisional origin,
    // so shift the whole subtree to the final one.
    if (absFixRight) {
        float newX = absFixCbx + absFixCbw - absFixRR - w - absFixMR;
        float dx = newX - x;
        if (dx != 0.0f) {
            x = newX;
            m_flowX = newX;
            for (auto* c : children) shiftStickySubtree(c, dx, 0.0f);
        }
    }
    if (absFixBottom) {
        float newY = absFixCby + absFixCbh - absFixRB - h - absFixMB;
        float dy = newY - y;
        if (dy != 0.0f) {
            y = newY;
            m_flowY = newY;
            for (auto* c : children) shiftStickySubtree(c, 0.0f, dy);
        }
    }
#endif

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
    if (style.display != CSS::Display::Flex && !style.explicitHeight.isSet()) {
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
    if (type == NodeType::Button && style.display != CSS::Display::Flex)
    {
        float btnContentH = h - pt - pb - bw * 2.0f;
        if (btnContentH > 0.0f)
        {
            float childTop = cy, childBottom = cy;
            bool any = false;
            for (auto* c : children)
            {
#ifdef MORPH_FEATURE_POSITION
                if (c->style.position == CSS::Position::Absolute
                    || c->style.position == CSS::Position::Fixed)
                    continue;
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
                        if (c->style.position == CSS::Position::Absolute
                    || c->style.position == CSS::Position::Fixed)
                    continue;
#endif
                        if (c->isWhitespaceOnly()) continue;
                        c->y += offset;
                        // Direct move outside any layout pass: repaint
                        // (display lists bake absolute coordinates).
                        markSubtreePaintDirty(c);
                    }
            }
        }
    }

    scrollEnabled = (style.overflow == CSS::Overflow::Scroll) ||
                    (style.overflow == CSS::Overflow::Auto && contentH > h);
#ifdef MORPH_FEATURE_POSITION
    // Sticky clamps resolve after heights (ours and the scrollport's) are
    // final. Each level resolves its direct sticky children; deeper ones
    // are covered transitively when their own parent resolves.
    for (auto* c : stickyChildren)
        c->updateStickySubtree();
#endif
    if (scrollEnabled) {
        if (scrollY > contentH - h) scrollY = contentH - h;
        if (scrollY < 0) scrollY = 0;
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
