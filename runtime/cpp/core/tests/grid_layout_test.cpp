// Grid layout test (headless, no window): real MorphNode::layout for
// display:grid — tracks, gaps, spans, areas, implicit tracks, minmax,
// repeat(auto-fill), content distribution and self alignment.
#include "../node.h"

#include <cmath>
#include <cstdio>

#if !defined(MORPH_FEATURE_GRID)
#error "grid_layout_test requires the GRID feature"
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
    return std::fabs(a - b) < 0.5f;
}

// Concrete node with rendering stripped out; layout runs for real.
struct LayoutNode : MorphNode
{
    void draw(Renderer&) override
    {
    }
};

static LayoutNode* makeItem(float w = -1.0f, float h = -1.0f)
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

static LayoutNode* makeGrid(const char* cols, const char* rows = "", float gap = 0.0f)
{
    LayoutNode* g = new LayoutNode();
    g->style.display = CSS::Display::Grid;
    g->style.gridTemplateColumns = cols ? cols : "";
    g->style.gridTemplateRows = rows ? rows : "";
    if (gap != 0.0f)
    {
        g->style.gap.value = gap;
    }
    return g;
}

static void attach(MorphNode* parent, MorphNode* child)
{
    child->parent = parent;
    parent->children.push_back(child);
}

static void layoutRoot(MorphNode* grid, float w = 600.0f)
{
    // Heap holder: stack destruction would cascade-delete the grid under
    // test (MorphNode owns children); the test reads geometry after.
    LayoutNode* holder = new LayoutNode();
    attach(holder, grid);
    holder->layout(0.0f, 0.0f, w, 0.0f, nullptr);
}

// Wave 1: fixed + fr tracks share free space.
static void testFrTracks()
{
    LayoutNode* g = makeGrid("100 1fr 2fr");
    LayoutNode* items[4];
    for (int i = 0; i < 4; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g);
    CHECK(near(items[0]->x, 0.0f), "fr col0 x");
    CHECK(near(items[0]->w, 100.0f), "fr col0 w");
    CHECK(near(items[1]->x, 100.0f), "fr col1 x");
    CHECK(near(items[1]->w, 500.0f / 3.0f), "fr col1 w");
    CHECK(near(items[2]->x, 100.0f + 500.0f / 3.0f), "fr col2 x");
    CHECK(near(items[2]->w, 1000.0f / 3.0f), "fr col2 w");
    // Fourth item wraps to row 2, first column.
    CHECK(near(items[3]->x, 0.0f), "fr wrap x");
    CHECK(near(items[3]->y, items[0]->y + items[0]->h), "fr wrap y");
}

// Uniform gap shrinks the fr pool.
static void testGap()
{
    LayoutNode* g = makeGrid("100 1fr 2fr", "", 8.0f);
    LayoutNode* items[3];
    for (int i = 0; i < 3; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g);
    float fr = (600.0f - 100.0f - 16.0f) / 3.0f;
    CHECK(near(items[1]->x, 108.0f), "gap col1 x");
    CHECK(near(items[1]->w, fr), "gap col1 w");
    CHECK(near(items[2]->x, 108.0f + fr + 8.0f), "gap col2 x");
}

// Wave 2: line placement, negative lines, spans.
static void testSpans()
{
    LayoutNode* g = makeGrid("150 1fr", "60 120 50", 10.0f);
    LayoutNode* head = makeItem();
    head->style.gridColumn = "1 / -1";
    LayoutNode* side = makeItem();
    side->style.gridRow = "2 / span 2";
    LayoutNode* main = makeItem();
    LayoutNode* foot = makeItem();
    attach(g, head);
    attach(g, side);
    attach(g, main);
    attach(g, foot);
    layoutRoot(g, 800.0f);
    // Inner width 800: cols 150 + 640 (1fr of 800-150-10).
    CHECK(near(head->x, 0.0f), "span head x");
    CHECK(near(head->w, 800.0f), "span head full width");
    CHECK(near(head->h, 60.0f), "span head h");
    CHECK(near(side->x, 0.0f), "span side x");
    CHECK(near(side->w, 150.0f), "span side w");
    CHECK(near(side->y, 70.0f), "span side y");
    CHECK(near(side->h, 180.0f), "span side spans rows 2-3");
    CHECK(near(main->x, 160.0f), "span main x");
    CHECK(near(main->w, 640.0f), "span main w");
    CHECK(near(main->y, 70.0f), "span main y");
    CHECK(near(main->h, 120.0f), "span main h");
    CHECK(near(foot->x, 160.0f), "span foot x");
    CHECK(near(foot->y, 200.0f), "span foot y");
    CHECK(near(foot->h, 50.0f), "span foot h");
}

// Named areas place by rectangle.
static void testAreas()
{
    LayoutNode* g = makeGrid("1fr 2fr", "50 100");
    g->style.gridTemplateAreas = "\"a a\" \"b c\"";
    LayoutNode* a = makeItem();
    a->style.gridArea = "a";
    LayoutNode* b = makeItem();
    b->style.gridArea = "b";
    LayoutNode* c = makeItem();
    c->style.gridArea = "c";
    attach(g, a);
    attach(g, b);
    attach(g, c);
    layoutRoot(g);
    CHECK(near(a->x, 0.0f), "area a x");
    CHECK(near(a->w, 600.0f), "area a full width");
    CHECK(near(a->h, 50.0f), "area a h");
    CHECK(near(b->x, 0.0f), "area b x");
    CHECK(near(b->w, 200.0f), "area b w");
    CHECK(near(b->y, 50.0f), "area b y");
    CHECK(near(c->x, 200.0f), "area c x");
    CHECK(near(c->w, 400.0f), "area c w");
    CHECK(near(c->y, 50.0f), "area c y");
}

// Implicit rows grow the grid; auto items flow row-wise.
static void testImplicit()
{
    LayoutNode* g = makeGrid("100 100", "", 8.0f);
    LayoutNode* items[5];
    for (int i = 0; i < 5; i++)
    {
        items[i] = makeItem(-1.0f, 40.0f);
        attach(g, items[i]);
    }
    layoutRoot(g);
    CHECK(near(items[2]->y, 48.0f), "implicit row2 y");
    CHECK(near(items[4]->x, 0.0f), "implicit row3 x");
    CHECK(near(items[4]->y, 96.0f), "implicit row3 y");
    CHECK(near(g->h, 40.0f * 3.0f + 16.0f), "implicit container h");
}

// minmax minimum floors fr distribution.
static void testMinmax()
{
    LayoutNode* g = makeGrid("minmax(100, 1fr) 1fr");
    LayoutNode* items[2];
    for (int i = 0; i < 2; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g, 150.0f);
    CHECK(near(items[0]->w, 100.0f), "minmax floor wins narrow");
    CHECK(near(items[1]->w, 50.0f), "minmax sibling takes rest");
}

// auto-fill repeats by the minimum.
static void testAutoFill()
{
    LayoutNode* g = makeGrid("repeat(auto-fill, minmax(100, 1fr))");
    LayoutNode* items[3];
    for (int i = 0; i < 3; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g, 350.0f);
    float w = 350.0f / 3.0f;
    CHECK(near(items[0]->w, w), "autofill track w");
    CHECK(near(items[1]->x, w), "autofill second x");
    CHECK(near(items[2]->x, w * 2.0f), "autofill third x");
}

// Content distribution packs free space.
static void testJustifyContent()
{
    LayoutNode* g = makeGrid("100 100");
    g->style.justifyContent = CSS::JustifyContent::Center;
    LayoutNode* items[2];
    for (int i = 0; i < 2; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g);
    CHECK(near(items[0]->x, 200.0f), "justify center leading");
    CHECK(near(items[1]->x, 300.0f), "justify center second");
}

// Named lines resolve placement.
static void testNamedLines()
{
    LayoutNode* g = makeGrid("[a] 100 [b] 1fr [c]");
    LayoutNode* it = makeItem();
    it->style.gridColumn = "b / -1";
    attach(g, it);
    layoutRoot(g);
    CHECK(near(it->x, 100.0f), "named line start");
    CHECK(near(it->w, 500.0f), "named line span");
}

// Self alignment overrides the container default.
static void testSelfAlign()
{
    LayoutNode* g = makeGrid("200", "100");
    LayoutNode* it = makeItem(50.0f, 20.0f);
    it->style.justifySelf = CSS::JustifySelf::Center;
    attach(g, it);
    layoutRoot(g);
    CHECK(near(it->x, 75.0f), "justify-self center");
    CHECK(near(it->w, 50.0f), "justify-self keeps width");
}

// Column flow fills columns first; explicit rows bound the wrap.
static void testColumnFlow()
{
    LayoutNode* g = makeGrid("100 100", "40 40");
    g->style.gridAutoFlow = CSS::GridAutoFlow::Column;
    LayoutNode* items[3];
    for (int i = 0; i < 3; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g);
    CHECK(near(items[0]->x, 0.0f), "colflow first x");
    CHECK(near(items[0]->y, 0.0f), "colflow first y");
    CHECK(near(items[1]->y, 40.0f), "colflow second row");
    CHECK(near(items[2]->x, 100.0f), "colflow wraps to col2");
    CHECK(near(items[2]->y, 0.0f), "colflow wrap row");
}

// Dense flow backfills holes left by spanning items.
static void testDenseFlow()
{
    LayoutNode* g = makeGrid("100 100", "", 0.0f);
    g->style.gridAutoFlow = CSS::GridAutoFlow::RowDense;
    LayoutNode* wide = makeItem();
    wide->style.gridColumn = "1 / span 2";
    LayoutNode* a = makeItem();
    LayoutNode* b = makeItem();
    attach(g, wide);
    attach(g, a);
    attach(g, b);
    layoutRoot(g);
    CHECK(near(wide->x, 0.0f), "dense wide x");
    CHECK(near(a->x, 0.0f), "dense second row x");
    CHECK(near(a->y, wide->y + wide->h), "dense second row y");
    CHECK(near(b->x, 100.0f), "dense backfills beside a");
    CHECK(near(b->y, a->y), "dense backfill row");
}

// Split gaps apply per axis; % tracks resolve against the inline size.
static void testSplitGapsPercent()
{
    LayoutNode* g = makeGrid("50% 50%", "40 40");
    g->style.rowGap.value = 20.0f;
    g->style.rowGapSet = true;
    g->style.columnGap.value = 10.0f;
    g->style.columnGapSet = true;
    LayoutNode* items[4];
    for (int i = 0; i < 4; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g);
    // Fixed/% tracks overflow rather than shrink for gaps: each 300,
    // the 10px gap pushes the second column to 310.
    CHECK(near(items[0]->w, 300.0f), "pct col w");
    CHECK(near(items[1]->x, 310.0f), "split col gap x");
    CHECK(near(items[2]->y, 60.0f), "split row gap y");
}

// fit-content caps the content contribution at its limit.
static void testFitContent()
{
    LayoutNode* g = makeGrid("fit-content(100) 1fr");
    LayoutNode* wide = makeItem(300.0f, 20.0f);
    LayoutNode* rest = makeItem();
    attach(g, wide);
    attach(g, rest);
    layoutRoot(g);
    CHECK(near(wide->w, 300.0f), "definite size overflows capped track");
    CHECK(near(rest->x, 100.0f), "fit-content sibling x");
    CHECK(near(rest->w, 500.0f), "fit-content sibling fills");
}

// auto-fit collapses empty tracks; the items share the whole width.
static void testAutoFitCollapse()
{
    LayoutNode* g = makeGrid("repeat(auto-fit, minmax(100, 1fr))", "", 8.0f);
    LayoutNode* items[2];
    for (int i = 0; i < 2; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g, 350.0f);
    // 3 repetitions fit (350+8)/(100+8), one collapses: (350-8)/2 each.
    CHECK(near(items[0]->w, 171.0f), "autofit collapse w");
    CHECK(near(items[1]->x, 179.0f), "autofit collapse second x");
    CHECK(near(items[1]->w, 171.0f), "autofit collapse second w");
}

// auto-fill keeps empty tracks (unlike auto-fit).
static void testAutoFillKeepsEmpty()
{
    LayoutNode* g = makeGrid("repeat(auto-fill, minmax(100, 1fr))", "", 8.0f);
    LayoutNode* items[2];
    for (int i = 0; i < 2; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g, 350.0f);
    // 3 tracks kept: (350-16)/3 each.
    float w = (350.0f - 16.0f) / 3.0f;
    CHECK(near(items[0]->w, w), "autofill keeps empty w");
    CHECK(near(items[1]->x, w + 8.0f), "autofill keeps empty second x");
}

// space-between spreads free space into the gaps.
static void testSpaceBetween()
{
    LayoutNode* g = makeGrid("100 100 100", "", 0.0f);
    g->style.justifyContent = CSS::JustifyContent::SpaceBetween;
    LayoutNode* items[3];
    for (int i = 0; i < 3; i++)
    {
        items[i] = makeItem();
        attach(g, items[i]);
    }
    layoutRoot(g);
    CHECK(near(items[0]->x, 0.0f), "space-between first x");
    CHECK(near(items[1]->x, 250.0f), "space-between middle x");
    CHECK(near(items[2]->x, 500.0f), "space-between last x");
}

int main()
{
    testFrTracks();
    testGap();
    testSpans();
    testAreas();
    testImplicit();
    testMinmax();
    testAutoFill();
    testJustifyContent();
    testNamedLines();
    testSelfAlign();
    testColumnFlow();
    testDenseFlow();
    testSplitGapsPercent();
    testFitContent();
    testAutoFitCollapse();
    testAutoFillKeepsEmpty();
    testSpaceBetween();
    if (failures == 0)
    {
        std::printf("grid-layout-test: %d checks passed\n", checks);
        return 0;
    }
    std::printf("grid-layout-test: %d/%d FAILED\n", failures, checks);
    return 1;
}
