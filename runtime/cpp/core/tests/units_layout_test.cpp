// CSS unit resolution test (headless, no window): every length unit
// resolves against its browser-grade base — px, % (containing block),
// em/ex/ch (element font, compounding), rem (root font lookup),
// vw/vh/vmin/vmax (viewport), physical units folded at parse.
#include "../node.h"

#include <cmath>
#include <cstdio>
#include <string>

#if !defined(MORPH_FEATURE_POSITION)
#error "units_layout_test requires the POSITION feature"
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

static LayoutNode* makeBox()
{
    return new LayoutNode();
}

static void attach(MorphNode* parent, MorphNode* child)
{
    child->parent = parent;
    parent->children.push_back(child);
}

static CssLength makeLen(float v, LengthUnit u)
{
    CssLength l;
    l.value = v;
    l.unit = u;
    return l;
}

// em compounds against the parent font; % font-size too.
static void testEmCompounding()
{
    LayoutNode holder;
    LayoutNode* parent = makeBox();
    parent->style.fontSize = pxLen(20.0f);
    LayoutNode* child = makeBox();
    child->style.fontSize = makeLen(150.0f, LengthUnit::Pct); // 30
    LayoutNode* grand = makeBox();
    grand->style.explicitWidth = makeLen(2.0f, LengthUnit::Em); // 2x30 = 60
    attach(&holder, parent);
    attach(parent, child);
    attach(child, grand);
    holder.layout(0.0f, 0.0f, 1000.0f, 600.0f, nullptr);
    CHECK(near(parent->m_computedFontSize, 20.0f), "px font-size sticks");
    CHECK(near(child->m_computedFontSize, 30.0f), "pct font-size vs parent");
    CHECK(near(grand->w, 60.0f), "em width compounds through pct font");
}

// rem ignores intermediate fonts and follows the root.
static void testRemRootLookup()
{
    LayoutNode holder;
    holder.style.fontSize = pxLen(20.0f); // root font = 20
    LayoutNode* mid = makeBox();
    mid->style.fontSize = pxLen(40.0f); // must not affect rem below
    LayoutNode* leaf = makeBox();
    leaf->style.explicitWidth = makeLen(2.0f, LengthUnit::Rem); // 40
    leaf->style.explicitHeight = makeLen(3.0f, LengthUnit::Rem); // 60
    attach(&holder, mid);
    attach(mid, leaf);
    holder.layout(0.0f, 0.0f, 1000.0f, 600.0f, nullptr);
    CHECK(near(holder.m_computedFontSize, 20.0f), "root px font resolves");
    CHECK(near(leaf->w, 40.0f), "rem width vs root font");
    CHECK(near(leaf->h, 60.0f), "rem height vs root font");
}

// rem on the root itself falls back to the 16px UA default.
static void testRemOnRoot()
{
    LayoutNode holder;
    holder.style.fontSize = makeLen(2.0f, LengthUnit::Rem); // 32
    holder.layout(0.0f, 0.0f, 1000.0f, 600.0f, nullptr);
    CHECK(near(holder.m_computedFontSize, 32.0f), "root rem vs UA default");
}

// Viewport units follow the window, not the containing block.
static void testViewportUnits()
{
    LayoutNode holder;
    LayoutNode* wrap = makeBox();
    wrap->style.explicitWidth = pxLen(100.0f); // narrow CB on purpose
    LayoutNode* a = makeBox();
    a->style.explicitWidth = makeLen(50.0f, LengthUnit::Vw); // 400 of 800
    LayoutNode* b = makeBox();
    b->style.explicitHeight = makeLen(25.0f, LengthUnit::Vh); // 150 of 600
    LayoutNode* c = makeBox();
    c->style.explicitWidth = makeLen(10.0f, LengthUnit::Vmin); // 60
    LayoutNode* d = makeBox();
    d->style.explicitWidth = makeLen(10.0f, LengthUnit::Vmax); // 80
    attach(&holder, wrap);
    attach(wrap, a);
    attach(wrap, b);
    attach(wrap, c);
    attach(wrap, d);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(near(a->w, 400.0f), "vw vs window width, not CB");
    CHECK(near(b->h, 150.0f), "vh vs window height");
    CHECK(near(c->w, 60.0f), "vmin vs smaller viewport side");
    CHECK(near(d->w, 80.0f), "vmax vs larger viewport side");
}

// % margins/padding resolve against the containing-block width;
// ex/ch fall back to 0.5em headless (no renderer to measure with).
static void testBoxUnits()
{
    LayoutNode holder;
    LayoutNode* child = makeBox();
    child->style.margin[3] = makeLen(10.0f, LengthUnit::Pct); // 100 of 1000
    child->style.padding[1] = makeLen(2.0f, LengthUnit::Em); // 2x16 = 32
    child->style.explicitWidth = makeLen(4.0f, LengthUnit::Ch); // 4x8 = 32
    child->style.explicitHeight = makeLen(2.0f, LengthUnit::Ex); // 2x8 = 16
    attach(&holder, child);
    holder.layout(0.0f, 0.0f, 1000.0f, 600.0f, nullptr);
    CHECK(near(child->x, 100.0f), "pct margin vs CB width");
    CHECK(near(child->w, 32.0f + 32.0f), "ch width + em padding");
    CHECK(near(child->h, 16.0f), "ex height falls back to 0.5em");
}

// min/max clamps accept relative units too.
static void testMinMaxUnits()
{
    LayoutNode holder;
    LayoutNode* child = makeBox();
    child->style.explicitWidth = pxLen(10.0f);
    child->style.minWidth = makeLen(5.0f, LengthUnit::Em); // 80 clamps up
    attach(&holder, child);
    holder.layout(0.0f, 0.0f, 1000.0f, 600.0f, nullptr);
    CHECK(near(child->w, 80.0f), "em min-width clamps");
}

// The shared CSS length parser: physical folding, case, rejections.
static void testParseCssLength()
{
    CssLength l;
    CHECK(parseCssLength("1in", l) && l.unit == LengthUnit::Px && near(l.value, 96.0f), "in folds to 96px");
    CHECK(parseCssLength("2.54cm", l) && near(l.value, 96.0f), "cm folds to px");
    CHECK(parseCssLength("72pt", l) && near(l.value, 96.0f), "pt folds to px");
    CHECK(parseCssLength("10VW", l) && l.unit == LengthUnit::Vw && near(l.value, 10.0f), "units case-insensitive");
    CHECK(parseCssLength("1.5em", l) && l.unit == LengthUnit::Em, "em parses");
    CHECK(parseCssLength("50%", l) && l.unit == LengthUnit::Pct, "pct parses");
    CHECK(!parseCssLength("auto", l), "auto rejected");
    CHECK(!parseCssLength("system", l), "bare words rejected");
    CHECK(!parseCssLength("", l), "empty rejected");
}

int main()
{
    testEmCompounding();
    testRemRootLookup();
    testRemOnRoot();
    testViewportUnits();
    testBoxUnits();
    testMinMaxUnits();
    testParseCssLength();
    std::printf("[units-layout-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
