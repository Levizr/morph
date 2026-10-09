// Position layout test (headless, no window): real MorphNode::layout,
// flatten and keyframe sampling for static/over-constrained/auto-margin
// absolute placement, % insets and widths, viewport sticky clamps and
// fixed-subtree scroll exemption.
#include "../node.h"

#include <cmath>
#include <cstdio>

#if !defined(MORPH_FEATURE_POSITION)
#error "position_layout_test requires the POSITION feature"
#endif

static int failures = 0;
static int checks = 0;

static void check(bool cond, const char* name, int line)
{
    checks++;
    if (!cond)
    {
        failures++;
        std::printf("FAIL %s (line %d)\n", name, line);
    }
}

#define CHECK(cond, name) check((cond), (name), __LINE__)

static bool near(float a, float b)
{
    return std::fabs(a - b) < 1e-3f;
}

// Concrete node with rendering stripped out; layout/flatten run for real.
struct LayoutNode : MorphNode
{
    void draw(Renderer&) override
    {
    }
};

static LayoutNode* makeBox(float w, float h)
{
    LayoutNode* n = new LayoutNode();
    if (w >= 0.0f)
    {
        n->style.explicitWidth.value = w;
    }
    if (h >= 0.0f)
    {
        n->style.explicitHeight.value = h;
    }
    return n;
}

static void attach(MorphNode* parent, MorphNode* child)
{
    child->parent = parent;
    parent->children.push_back(child);
}

// Absolute anchored offsets place against the containing block.
static void testAbsAnchored()
{
    LayoutNode root;
    root.layout(0.0f, 0.0f, 1000.0f, 0.0f, nullptr);
    (void)root;
    LayoutNode* cb = makeBox(-1.0f, 72.0f);
    cb->style.position = CSS::Position::Relative;
    LayoutNode* abs = makeBox(200.0f, 40.0f);
    abs->style.position = CSS::Position::Absolute;
    abs->style.left.value = 24.0f;
    abs->style.top.value = 8.0f;
    LayoutNode holder;
    attach(&holder, cb);
    attach(cb, abs);
    holder.layout(0.0f, 0.0f, 1000.0f, 0.0f, nullptr);
    CHECK(near(abs->x, cb->x + 24.0f), "abs left anchors to CB");
    CHECK(near(abs->y, cb->y + 8.0f), "abs top anchors to CB");
    CHECK(near(abs->w, 200.0f), "abs keeps explicit width");
}

// All-auto absolute holds its static (flow-slot) position.
static void testAbsStatic()
{
    LayoutNode holder;
    LayoutNode* cb = makeBox(-1.0f, 72.0f);
    cb->style.position = CSS::Position::Relative;
    LayoutNode* spacer = makeBox(-1.0f, 20.0f);
    LayoutNode* abs = makeBox(200.0f, 40.0f);
    abs->style.position = CSS::Position::Absolute;
    attach(&holder, cb);
    attach(cb, spacer);
    attach(cb, abs);
    holder.layout(0.0f, 0.0f, 1000.0f, 0.0f, nullptr);
    CHECK(near(abs->x, cb->x), "all-auto abs keeps CB left");
    CHECK(near(abs->y, spacer->y + spacer->h + spacer->m_computedMargin[2]), "all-auto abs keeps flow slot");
}

// Over-constrained keeps width and ignores the far offset (LTR).
static void testAbsOverConstrained()
{
    LayoutNode holder;
    LayoutNode* cb = makeBox(-1.0f, 72.0f);
    cb->style.position = CSS::Position::Relative;
    LayoutNode* abs = makeBox(100.0f, 40.0f);
    abs->style.position = CSS::Position::Absolute;
    abs->style.left.value = 10.0f;
    abs->style.right.value = 10.0f;
    abs->style.top.value = 10.0f;
    attach(&holder, cb);
    attach(cb, abs);
    holder.layout(0.0f, 0.0f, 1000.0f, 0.0f, nullptr);
    CHECK(near(abs->w, 100.0f), "over-constrained keeps width");
    CHECK(near(abs->x, cb->x + 10.0f), "over-constrained keeps near offset");
}

// left:0 + right:0 + auto margins centers; overflow left-sticks.
static void testAbsAutoMargins()
{
    LayoutNode holder;
    LayoutNode* cb = makeBox(-1.0f, 72.0f);
    cb->style.position = CSS::Position::Relative;
    LayoutNode* abs = makeBox(100.0f, 40.0f);
    abs->style.position = CSS::Position::Absolute;
    abs->style.left.value = 0.0f;
    abs->style.right.value = 0.0f;
    abs->style.marginAuto[3] = true;
    abs->style.marginAuto[1] = true;
    attach(&holder, cb);
    attach(cb, abs);
    holder.layout(0.0f, 0.0f, 1000.0f, 0.0f, nullptr);
    CHECK(near(abs->x, cb->x + (cb->w - 100.0f) * 0.5f), "auto margins center");

    LayoutNode holder2;
    LayoutNode* cb2 = makeBox(-1.0f, 72.0f);
    cb2->style.position = CSS::Position::Relative;
    LayoutNode* wide = makeBox(1200.0f, 40.0f);
    wide->style.position = CSS::Position::Absolute;
    wide->style.left.value = 0.0f;
    wide->style.right.value = 0.0f;
    wide->style.marginAuto[3] = true;
    wide->style.marginAuto[1] = true;
    attach(&holder2, cb2);
    attach(cb2, wide);
    holder2.layout(0.0f, 0.0f, 1000.0f, 0.0f, nullptr);
    CHECK(near(wide->x, cb2->x), "overflowing auto margins left-stick");
}

// Percentages resolve against the containing block.
static void testPercentInsets()
{
    LayoutNode holder;
    LayoutNode* cb = makeBox(-1.0f, 120.0f);
    cb->style.position = CSS::Position::Relative;
    LayoutNode* abs = makeBox(100.0f, 40.0f);
    abs->style.position = CSS::Position::Absolute;
    abs->style.left.value = 50.0f;
    abs->style.left.unit = LengthUnit::Pct;
    abs->style.top.value = 50.0f;
    abs->style.top.unit = LengthUnit::Pct;
    LayoutNode* half = makeBox(-1.0f, 40.0f);
    half->style.position = CSS::Position::Absolute;
    half->style.explicitWidth.value = 50.0f;
    half->style.explicitWidth.unit = LengthUnit::Pct;
    attach(&holder, cb);
    attach(cb, abs);
    attach(cb, half);
    holder.layout(0.0f, 0.0f, 1000.0f, 0.0f, nullptr);
    CHECK(near(abs->x, cb->x + cb->w * 0.5f), "left % resolves against CB width");
    CHECK(near(abs->y, cb->y + cb->h * 0.5f), "top % resolves against CB height");
    CHECK(near(half->w, cb->w * 0.5f), "width % resolves against CB width");
}

// Relative % offsets shift against the parent box.
static void testRelativePercent()
{
    LayoutNode holder;
    LayoutNode* rel = makeBox(-1.0f, 40.0f);
    rel->style.position = CSS::Position::Relative;
    rel->style.left.value = 10.0f;
    rel->style.left.unit = LengthUnit::Pct;
    attach(&holder, rel);
    holder.layout(0.0f, 0.0f, 1000.0f, 0.0f, nullptr);
    CHECK(near(rel->x, holder.x + 100.0f), "relative left % shifts by CB width share");
}

// Viewport sticky without a scroll ancestor clamps to the window box.
static void testStickyViewport()
{
    LayoutNode holder;
    LayoutNode* sticky = makeBox(-1.0f, 40.0f);
    sticky->style.position = CSS::Position::Sticky;
    sticky->style.top.value = 0.0f;
    attach(&holder, sticky);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(near(sticky->y, holder.y), "viewport sticky rests at flow slot");
}

// Sticky inside a scroller clamps against scrolled content.
static void testStickyScroller()
{
    LayoutNode holder;
    LayoutNode* scroller = makeBox(-1.0f, 100.0f);
    scroller->style.overflow = CSS::Overflow::Scroll;
    LayoutNode* sticky = makeBox(-1.0f, 40.0f);
    sticky->style.position = CSS::Position::Sticky;
    sticky->style.top.value = 0.0f;
    LayoutNode* tall = makeBox(-1.0f, 300.0f);
    attach(&holder, scroller);
    attach(scroller, sticky);
    attach(scroller, tall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(scroller->scrollEnabled, "tall content enables scroll");
    CHECK(near(sticky->y, scroller->y), "sticky rests at flow slot");
    scroller->scrollY = 50.0f;
    scroller->updateStickySubtree();
    CHECK(near(sticky->y, scroller->y + 50.0f), "sticky clamps to scrolled viewport top");
}

// Fixed subtrees are flagged, flatten viewport-locked under scroll, and
// survive culling that drops equivalent scrolling content.
static void testFixedExemption()
{
    LayoutNode holder;
    LayoutNode* scroller = makeBox(-1.0f, 100.0f);
    scroller->style.overflow = CSS::Overflow::Scroll;
    LayoutNode* tall = makeBox(-1.0f, 300.0f);
    LayoutNode* fixed = makeBox(100.0f, 40.0f);
    fixed->style.position = CSS::Position::Fixed;
    fixed->style.top.value = 10.0f;
    fixed->style.left.value = 10.0f;
    fixed->style.overflow = CSS::Overflow::Hidden;
    attach(&holder, scroller);
    attach(scroller, tall);
    attach(scroller, fixed);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(scroller->m_subtreeHasFixed, "fixed child flags ancestors");
    CHECK(!tall->m_subtreeHasFixed, "plain child unflagged");
    scroller->scrollY = 1000.0f;
    RenderFrame frame;
    frame.viewW = 800.0f;
    frame.viewH = 600.0f;
    holder.flatten(frame, -1, 0.0f);
    const FlatRenderNode* fixedFn = nullptr;
    for (auto& fn : frame.nodes) {
        if (near(fn.w, 100.0f) && near(fn.h, 40.0f))
            fixedFn = &fn;
    }
    CHECK(fixedFn != nullptr, "scrolled-away fixed node is not culled");
    if (fixedFn) {
        CHECK(near(fixedFn->y, fixed->y), "fixed frame keeps layout position");
        CHECK(fixedFn->hasFixedSubtree, "fixed frame flagged");
    }
    CHECK(!frame.nodes.empty() && frame.nodes[0].hasFixedSubtree, "flag propagates to ancestors");
}

// Bottom-only boxes with auto height count back from the far edge using
// the final (content-grown) height, keeping content glued (CSS 2.1 §10.6.7).
static void testAbsBottomAutoHeight()
{
    LayoutNode holder;
    LayoutNode* cb = makeBox(-1.0f, 200.0f);
    cb->style.position = CSS::Position::Relative;
    LayoutNode* abs = makeBox(100.0f, -1.0f);
    abs->style.position = CSS::Position::Absolute;
    abs->style.bottom.value = 24.0f;
    LayoutNode* inner = makeBox(-1.0f, 40.0f);
    attach(&holder, cb);
    attach(cb, abs);
    attach(abs, inner);
    holder.layout(0.0f, 0.0f, 1000.0f, 600.0f, nullptr);
    CHECK(near(abs->h, 40.0f), "bottom-anchored auto height grows from content");
    CHECK(near(abs->y, cb->y + cb->h - 24.0f - abs->h), "bottom-only counts back with final height");
    CHECK(near(inner->y, abs->y), "bottom fix keeps content glued");
}

// Same rule for viewport-anchored fixed boxes (the fixed-badge case).
static void testFixedBottomAutoHeight()
{
    LayoutNode holder;
    LayoutNode* fixed = makeBox(100.0f, -1.0f);
    fixed->style.position = CSS::Position::Fixed;
    fixed->style.right.value = 24.0f;
    fixed->style.bottom.value = 24.0f;
    LayoutNode* inner = makeBox(-1.0f, 40.0f);
    attach(&holder, fixed);
    attach(fixed, inner);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(near(fixed->h, 40.0f), "fixed auto height grows from content");
    CHECK(near(fixed->y, 600.0f - 24.0f - fixed->h), "fixed bottom counts back with final height");
    CHECK(near(fixed->x, 800.0f - 24.0f - fixed->w), "fixed right counts back from viewport");
    CHECK(near(inner->y, fixed->y), "fixed bottom fix keeps content glued");
}

// Bottom-only with an explicit height still moves when a min-height clamp
// grows the box afterwards.
#ifdef MORPH_FEATURE_MIN_MAX
static void testAbsBottomMinHeight()
{
    LayoutNode holder;
    LayoutNode* cb = makeBox(-1.0f, 200.0f);
    cb->style.position = CSS::Position::Relative;
    LayoutNode* abs = makeBox(100.0f, 10.0f);
    abs->style.position = CSS::Position::Absolute;
    abs->style.bottom.value = 24.0f;
    abs->style.minHeight.value = 60.0f;
    attach(&holder, cb);
    attach(cb, abs);
    holder.layout(0.0f, 0.0f, 1000.0f, 600.0f, nullptr);
    CHECK(near(abs->h, 60.0f), "min-height clamp grows bottom-anchored box");
    CHECK(near(abs->y, cb->y + cb->h - 24.0f - 60.0f), "bottom-only follows min-height growth");
}
#endif

// Lengths interpolate per unit; unset or mixed px/% snaps to target.
static void testLengthLerp()
{
    MorphStyle a;
    MorphStyle b;
    b.left.value = 100.0f;
    b.explicitWidth.value = 50.0f;
    b.explicitWidth.unit = LengthUnit::Pct;
    MorphStyle out;
    MorphNode::interpolateStyles(out, a, b, 0.5f);
    CHECK(out.left.isSet() && !out.left.isPercent(), "unset left snaps to target");
    CHECK(near(out.left.value, 100.0f), "snapped left takes target value");
    CHECK(out.explicitWidth.isSet() && out.explicitWidth.isPercent(), "unset width snaps to target");
    CHECK(near(out.explicitWidth.value, 50.0f), "snapped width keeps target value");
    MorphStyle c;
    c.left.value = 0.0f;
    MorphStyle d;
    d.left.value = 100.0f;
    MorphNode::interpolateStyles(out, c, d, 0.5f);
    CHECK(!out.left.isPercent() && near(out.left.value, 50.0f), "px insets lerp");
}

int main()
{
    testAbsAnchored();
    testAbsStatic();
    testAbsOverConstrained();
    testAbsAutoMargins();
    testPercentInsets();
    testRelativePercent();
    testStickyViewport();
    testStickyScroller();
    testFixedExemption();
    testAbsBottomAutoHeight();
    testFixedBottomAutoHeight();
#ifdef MORPH_FEATURE_MIN_MAX
    testAbsBottomMinHeight();
#endif
    testLengthLerp();
    std::printf("[position-layout-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
