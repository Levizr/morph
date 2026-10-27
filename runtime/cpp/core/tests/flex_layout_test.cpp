// Flex layout test (headless, no window): drives the real
// MorphNode::layout through grow/shrink, gap distribution and margins.
// These cover the invariants that keep auto-sized flex rows from
// overflowing their stretched parent.
#include "../node.h"

#include <cmath>
#include <cstdio>

#if !defined(MORPH_FEATURE_FLEX)
#error "flex_layout_test requires the FLEX feature"
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
    return std::fabs(a - b) < 1e-2f;
}

// Concrete node with rendering stripped out; layout runs for real.
struct LayoutNode : MorphNode
{
    void draw(Renderer&) override
    {
    }
};

static CssLength px(float v)
{
    CssLength l;
    l.value = v;
    l.unit = LengthUnit::Px;
    return l;
}

static CssLength pct(float v)
{
    CssLength l;
    l.value = v;
    l.unit = LengthUnit::Pct;
    return l;
}

struct IntrinsicNode : LayoutNode
{
    float intrinsic = 0.0f;
    float contentWidth(Renderer*) override
    {
        return intrinsic;
    }
};

static LayoutNode* box(float w, float h)
{
    LayoutNode* n = new LayoutNode();
    n->style.explicitWidth = px(w);
    n->style.explicitHeight = px(h);
    return n;
}

static IntrinsicNode* intrinsic(float w)
{
    IntrinsicNode* n = new IntrinsicNode();
    n->intrinsic = w;
    return n;
}

static void attach(MorphNode* parent, MorphNode* child)
{
    child->parent = parent;
    parent->children.push_back(child);
}

static void rowContainer(LayoutNode& n, float w, float h)
{
    n.style.display = CSS::Display::Flex;
    n.style.flexDirection = CSS::FlexDirection::Row;
    n.style.explicitWidth = px(w);
    n.style.explicitHeight = px(h);
}

static void columnContainer(LayoutNode& n, float w, float h)
{
    n.style.display = CSS::Display::Flex;
    n.style.flexDirection = CSS::FlexDirection::Column;
    n.style.explicitWidth = px(w);
    n.style.explicitHeight = px(h);
}

// Grow consumes the free space; space-between must not distribute that
// same space a second time (which pushes the trailing item off-screen).
static void testGrowWithSpaceBetween()
{
    LayoutNode flex;
    rowContainer(flex, 300.0f, 100.0f);
    flex.style.justifyContent = CSS::JustifyContent::SpaceBetween;
    LayoutNode* grow = box(100.0f, 50.0f);
    grow->style.flexGrow = 1.0f;
    LayoutNode* tail = box(50.0f, 50.0f);
    attach(&flex, grow);
    attach(&flex, tail);

    flex.layout(0.0f, 0.0f, 300.0f, 100.0f, nullptr);

    CHECK(near(grow->w, 250.0f), "grow fills the remaining main size");
    CHECK(near(tail->x, 250.0f), "space-between packs the tail at the end");
    CHECK(near(tail->x + tail->w, 300.0f), "grow + space-between does not overflow");
}

// Flex positions a row item at its margin box origin and layout adds the
// margin. Adding it in both places doubled margins and shifted content.
static void testRowMarginAppliedOnce()
{
    LayoutNode flex;
    rowContainer(flex, 200.0f, 100.0f);
    LayoutNode* a = box(50.0f, 50.0f);
    LayoutNode* b = box(50.0f, 50.0f);
    a->style.margin[3] = px(10.0f);
    attach(&flex, a);
    attach(&flex, b);

    flex.layout(0.0f, 0.0f, 200.0f, 100.0f, nullptr);

    CHECK(near(a->x, 10.0f), "row margin-left applies exactly once");
    CHECK(near(b->x, 60.0f), "row cursor advances past the margin once");
}

// Explicit sizes are flex bases, not final sizes. The final layout pass
// must keep the shrunken border-box instead of re-applying width/height.
static void testShrinkKeepsExplicitFlexSize()
{
    LayoutNode flex;
    rowContainer(flex, 100.0f, 100.0f);
    LayoutNode* a = box(80.0f, 50.0f);
    LayoutNode* b = box(80.0f, 50.0f);
    attach(&flex, a);
    attach(&flex, b);

    flex.layout(0.0f, 0.0f, 100.0f, 100.0f, nullptr);

    CHECK(near(a->w, 50.0f), "explicit width shrinks to its flex share");
    CHECK(near(b->w, 50.0f), "second item shrinks to its flex share");
    CHECK(near(b->x, 50.0f), "shrunk item starts after the first share");
    CHECK(near(b->x + b->w, 100.0f), "shrunk row fits its container");
}

static void testColumnShrinkKeepsExplicitFlexSize()
{
    LayoutNode flex;
    columnContainer(flex, 100.0f, 100.0f);
    LayoutNode* a = box(40.0f, 80.0f);
    LayoutNode* b = box(40.0f, 80.0f);
    attach(&flex, a);
    attach(&flex, b);

    flex.layout(0.0f, 0.0f, 100.0f, 100.0f, nullptr);

    CHECK(near(a->h, 50.0f), "explicit height shrinks to its flex share");
    CHECK(near(b->h, 50.0f), "second column item shrinks to its share");
    CHECK(near(b->y, 50.0f), "shrunk column item stacks after the first");
    CHECK(near(b->y + b->h, 100.0f), "shrunk column fits its container");
}

// An empty auto-width flex item (e.g. a reactive `<span>{cond ? "On" : ""}</span>`)
// has zero intrinsic width. It must not keep the parent-fill width from the
// provisional layout and steal space from its sibling, which used to wrap the
// sibling's label text.
static void testEmptyAutoItemDoesNotStealWidth()
{
    LayoutNode flex;
    rowContainer(flex, 400.0f, 50.0f);
    IntrinsicNode* label = intrinsic(100.0f);
    IntrinsicNode* state = intrinsic(0.0f);
    attach(&flex, label);
    attach(&flex, state);

    flex.layout(0.0f, 0.0f, 400.0f, 50.0f, nullptr);

    CHECK(near(label->w, 100.0f), "label keeps its intrinsic width");
    CHECK(near(state->w, 0.0f), "empty auto item stays zero-width");
    CHECK(near(state->x, 100.0f), "empty item does not consume sibling space");
}

// A column flex container with auto height has an indefinite main axis.
// Its children must keep their natural heights; treating the provisional
// zero height as available space used to shrink them all away (e.g. the
// vertical arms of the FAB's multi-color plus).
static void testAutoHeightColumnKeepsIntrinsicHeights()
{
    LayoutNode flex;
    flex.style.display = CSS::Display::Flex;
    flex.style.flexDirection = CSS::FlexDirection::Column;
    flex.style.explicitWidth = px(15.0f);
    LayoutNode* top = box(5.0f, 10.0f);
    LayoutNode* mid = box(15.0f, 5.0f);
    LayoutNode* bottom = box(5.0f, 10.0f);
    attach(&flex, top);
    attach(&flex, mid);
    attach(&flex, bottom);

    flex.layout(0.0f, 0.0f, 56.0f, 0.0f, nullptr);

    CHECK(near(top->h, 10.0f), "auto-height column keeps first child height");
    CHECK(near(mid->h, 5.0f), "auto-height column keeps middle child height");
    CHECK(near(bottom->h, 10.0f), "auto-height column keeps last child height");
    CHECK(near(flex.h, 25.0f), "auto-height column grows to the sum of children");

    // The same column as a centered item inside a fixed FAB row must retain
    // its intrinsic width and height.
    LayoutNode* fab = new LayoutNode();
    rowContainer(*fab, 56.0f, 56.0f);
    fab->style.alignItems = CSS::AlignItems::Center;
    fab->style.justifyContent = CSS::JustifyContent::Center;
    LayoutNode* plus = new LayoutNode();
    plus->style.display = CSS::Display::Flex;
    plus->style.flexDirection = CSS::FlexDirection::Column;
    plus->style.justifyContent = CSS::JustifyContent::Center;
    plus->style.alignItems = CSS::AlignItems::Center;
    LayoutNode* pTop = box(5.0f, 10.0f);
    LayoutNode* pMid = box(15.0f, 5.0f);
    LayoutNode* pBot = box(5.0f, 10.0f);
    attach(plus, pTop);
    attach(plus, pMid);
    attach(plus, pBot);
    attach(fab, plus);
    fab->layout(0.0f, 0.0f, 56.0f, 56.0f, nullptr);
    CHECK(near(plus->w, 15.0f), "centered auto-width column keeps intrinsic width");
    CHECK(near(plus->h, 25.0f), "centered auto-height column keeps intrinsic height");
    CHECK(near(plus->x, 20.5f), "column is centered horizontally");
    CHECK(near(plus->y, 15.5f), "column is centered vertically");
    CHECK(near(pTop->h, 10.0f), "nested first arm keeps height");
    CHECK(near(pMid->h, 5.0f), "nested middle arm keeps height");
    CHECK(near(pBot->h, 10.0f), "nested last arm keeps height");
}

// The window root establishes a definite viewport height. A percentage-height
// shell must resolve it on every full relayout, not just the first pass (the
// second pass was skipped once the tree was already clean).
static void testRootPercentHeightSurvivesRelayout()
{
    LayoutNode root;
    LayoutNode* app = new LayoutNode();
    app->style.explicitWidth = pct(100.0f);
    app->style.explicitHeight = pct(100.0f);
    app->style.display = CSS::Display::Flex;
    app->style.flexDirection = CSS::FlexDirection::Column;
    LayoutNode* search = box(0.0f, 52.0f);
    LayoutNode* list = new LayoutNode();
    list->style.flexGrow = 1.0f;
    attach(app, search);
    attach(app, list);
    attach(&root, app);

    root.layoutIfNeeded(0.0f, 0.0f, 400.0f, 740.0f, nullptr);
    CHECK(near(app->h, 740.0f), "percent shell fills the viewport on first layout");
    CHECK(near(search->h, 52.0f), "fixed search bar keeps its height on first layout");

    // Full relayout (window uncovered / sibling closed) used to leave the
    // app shell at zero because the root's first child pass saw parentH 0.
    root.markDirty(SubtreeDirty);
    root.layoutIfNeeded(0.0f, 0.0f, 400.0f, 740.0f, nullptr);
    CHECK(near(app->h, 740.0f), "percent shell keeps viewport height on relayout");
    CHECK(near(search->h, 52.0f), "fixed search bar survives a full relayout");
}

// A stretched column item fills the cross axis minus its margins.
static void testColumnStretchWithMargins()
{
    LayoutNode flex;
    columnContainer(flex, 300.0f, 100.0f);
    LayoutNode* child = new LayoutNode();
    child->style.explicitHeight = px(50.0f);
    child->style.margin[3] = px(10.0f);
    child->style.margin[1] = px(10.0f);
    attach(&flex, child);

    flex.layout(0.0f, 0.0f, 300.0f, 100.0f, nullptr);

    CHECK(near(child->x, 10.0f), "stretched column child starts after its left margin");
    CHECK(near(child->w, 280.0f), "stretched width subtracts both horizontal margins");
}

int main()
{
    testGrowWithSpaceBetween();
    testRowMarginAppliedOnce();
    testShrinkKeepsExplicitFlexSize();
    testColumnShrinkKeepsExplicitFlexSize();
    testEmptyAutoItemDoesNotStealWidth();
    testAutoHeightColumnKeepsIntrinsicHeights();
    testColumnStretchWithMargins();
    testRootPercentHeightSurvivesRelayout();
    std::printf("[flex-layout-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
