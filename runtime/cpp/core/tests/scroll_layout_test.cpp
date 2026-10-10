// Scroll layout test (headless, no window): real MorphNode::layout,
// per-axis overflow enables, wheel chaining (deepest-first, remainder
// bubbles outward), horizontal wheel, shift+wheel, clamps, page scroll
// and scrollbar thumb geometry.
#include "../node.h"

#include <cmath>
#include <cstdio>

#if !defined(MORPH_FEATURE_SCROLL)
#error "scroll_layout_test requires the SCROLL feature"
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

// Concrete node with rendering stripped out; layout/events run for real.
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

static MorphEvent wheel(float dx, float dy, int mods = 0)
{
    MorphEvent e;
    e.type = EventType::Scroll;
    e.scrollX = dx;
    e.scroll = dy;
    e.mods = mods;
    return e;
}

// Vertical overflow arms only the Y axis.
static void testVerticalOnly()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowY = CSS::Overflow::Auto;
    LayoutNode* tall = makeBox(-1.0f, 300.0f);
    attach(&holder, box);
    attach(box, tall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(box->scrollYEnabled, "tall content arms Y");
    CHECK(!box->scrollXEnabled, "fitting width leaves X disarmed");
    CHECK(near(box->contentH, 300.0f), "contentH tracks tall child");
    CHECK(near(box->contentW, box->w), "contentW clamps to box width");
}

// Horizontal overflow arms only the X axis.
static void testHorizontalOnly()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowX = CSS::Overflow::Auto;
    LayoutNode* wide = makeBox(500.0f, 20.0f);
    attach(&holder, box);
    attach(box, wide);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(box->scrollXEnabled, "wide content arms X");
    CHECK(!box->scrollYEnabled, "fitting height leaves Y disarmed");
    CHECK(near(box->contentW, 500.0f), "contentW tracks wide child");
}

// overflow:hidden clips without scrolling either axis.
static void testHiddenDisarms()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowX = CSS::Overflow::Hidden;
    box->style.overflowY = CSS::Overflow::Hidden;
    LayoutNode* big = makeBox(500.0f, 300.0f);
    attach(&holder, box);
    attach(box, big);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(!box->scrollXEnabled && !box->scrollYEnabled, "hidden never arms scroll");
    CHECK(box->clipsOverflowBox(), "hidden still clips");
}

// overflow:scroll arms the axis even when content fits.
static void testScrollArmsWithoutOverflow()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowY = CSS::Overflow::Scroll;
    LayoutNode* small = makeBox(-1.0f, 20.0f);
    attach(&holder, box);
    attach(box, small);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(box->scrollYEnabled, "scroll arms Y without overflow");
    CHECK(near(box->scrollY, 0.0f), "unscrollable axis stays at zero");
    float p, s;
    box->vScrollThumb(box->h, &p, &s);
    CHECK(near(s, box->h) && near(p, 0.0f), "fitting thumb fills track");
}

// overflow:clip behaves like hidden (clips, never scrolls).
static void testClipDisarms()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowX = CSS::Overflow::Clip;
    box->style.overflowY = CSS::Overflow::Clip;
    LayoutNode* big = makeBox(500.0f, 300.0f);
    attach(&holder, box);
    attach(box, big);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(!box->scrollXEnabled && !box->scrollYEnabled, "clip never arms scroll");
    CHECK(box->clipsOverflowBox(), "clip still clips");
}

// Wheel scrolls the box under the cursor and consumes the delta.
static void testWheelScrolls()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowY = CSS::Overflow::Auto;
    LayoutNode* tall = makeBox(-1.0f, 300.0f);
    attach(&holder, box);
    attach(box, tall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    MorphEvent e = wheel(0.0f, -1.0f);
    bool handled = holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f);
    CHECK(handled, "consumed wheel reports handled");
    CHECK(near(box->scrollY, 40.0f), "wheel down scrolls one step");
    CHECK(near(e.scroll, 0.0f), "consumed axis zeroes its delta");
}

// Wheel over an exhausted inner scroller chains to the outer one.
static void testWheelChaining()
{
    LayoutNode holder;
    LayoutNode* outer = makeBox(300.0f, 200.0f);
    outer->style.overflowY = CSS::Overflow::Auto;
    LayoutNode* inner = makeBox(200.0f, 100.0f);
    inner->style.overflowY = CSS::Overflow::Auto;
    LayoutNode* tall = makeBox(-1.0f, 400.0f);
    // Tall sibling makes the outer box itself overflow, so the chained
    // remainder has somewhere to go.
    LayoutNode* outerTall = makeBox(-1.0f, 250.0f);
    attach(&holder, outer);
    attach(outer, inner);
    attach(inner, tall);
    attach(outer, outerTall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    float px = inner->x + 10.0f, py = inner->y + 10.0f;
    // Inner has room: it consumes, outer stays put.
    MorphEvent e1 = wheel(0.0f, -1.0f);
    CHECK(holder.dispatchEvent(e1, px, py), "inner wheel handled");
    CHECK(near(inner->scrollY, 40.0f), "inner consumes first");
    CHECK(near(outer->scrollY, 0.0f), "outer untouched while inner scrolls");
    // Exhaust the inner box, then wheel again: remainder chains outward.
    inner->scrollY = inner->contentH - inner->h;
    MorphEvent e2 = wheel(0.0f, -1.0f);
    CHECK(holder.dispatchEvent(e2, px, py), "chained wheel handled");
    CHECK(near(inner->scrollY, inner->contentH - inner->h), "exhausted inner holds its limit");
    CHECK(near(outer->scrollY, 40.0f), "remainder chains to outer");
}

// Horizontal wheel deltas scroll the X axis only.
static void testWheelHorizontal()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowX = CSS::Overflow::Auto;
    LayoutNode* wide = makeBox(500.0f, 20.0f);
    attach(&holder, box);
    attach(box, wide);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    MorphEvent e = wheel(-1.0f, 0.0f);
    CHECK(holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f), "horizontal wheel handled");
    CHECK(near(box->scrollX, 40.0f), "horizontal wheel scrolls X");
    CHECK(near(box->scrollY, 0.0f), "Y untouched by X-only wheel");
}

// Shift+wheel maps a vertical delta onto the horizontal axis.
static void testShiftWheel()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowX = CSS::Overflow::Auto;
    LayoutNode* wide = makeBox(500.0f, 20.0f);
    attach(&holder, box);
    attach(box, wide);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    MorphEvent e = wheel(0.0f, -1.0f, 0x01);
    CHECK(holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f), "shift+wheel handled");
    CHECK(near(box->scrollX, 40.0f), "shift+wheel scrolls X");
}

// Scroll offsets clamp to the content range on both axes.
static void testClamps()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowX = CSS::Overflow::Auto;
    box->style.overflowY = CSS::Overflow::Auto;
    LayoutNode* big = makeBox(500.0f, 300.0f);
    attach(&holder, box);
    attach(box, big);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    for (int i = 0; i < 20; i++) {
        MorphEvent e = wheel(-1.0f, -1.0f);
        holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f);
    }
    CHECK(near(box->scrollX, box->contentW - box->w), "scrollX clamps at max");
    CHECK(near(box->scrollY, box->contentH - box->h), "scrollY clamps at max");
    for (int i = 0; i < 20; i++) {
        MorphEvent e = wheel(1.0f, 1.0f);
        holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f);
    }
    CHECK(near(box->scrollX, 0.0f), "scrollX clamps at zero");
    CHECK(near(box->scrollY, 0.0f), "scrollY clamps at zero");
}

// A page taller than the window scrolls at the root (viewport
// propagation): the root clamps to the viewport and arms Y.
static void testPageScroll()
{
    LayoutNode holder;
    LayoutNode* tall = makeBox(-1.0f, 900.0f);
    attach(&holder, tall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(near(holder.h, 600.0f), "root clamps to viewport height");
    CHECK(holder.scrollYEnabled, "overflowing page arms root scroll");
    MorphEvent e = wheel(0.0f, -1.0f);
    CHECK(holder.dispatchEvent(e, 400.0f, 300.0f), "page wheel handled");
    CHECK(near(holder.scrollY, 40.0f), "page scrolls under wheel");
}

// Thumb geometry: proportional size, zero origin, end clamp.
static void testThumbGeometry()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowX = CSS::Overflow::Auto;
    box->style.overflowY = CSS::Overflow::Auto;
    LayoutNode* big = makeBox(400.0f, 300.0f);
    attach(&holder, box);
    attach(box, big);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    float p, s;
    box->vScrollThumb(box->h, &p, &s);
    CHECK(near(s, box->h * box->h / box->contentH), "v thumb proportional");
    CHECK(near(p, 0.0f), "v thumb starts at zero");
    box->scrollY = box->contentH - box->h;
    box->vScrollThumb(box->h, &p, &s);
    CHECK(near(p + s, box->h), "v thumb ends at track end");
    box->hScrollThumb(box->w, &p, &s);
    CHECK(near(s, box->w * box->w / box->contentW), "h thumb proportional");
    box->scrollX = box->contentW - box->w;
    box->hScrollThumb(box->w, &p, &s);
    CHECK(near(p + s, box->w), "h thumb ends at track end");
}

// `overflow: hidden` on the body locks page scroll (viewport
// propagation, like browsers) — the manual off switch for pages.
static void testBodyHiddenDisablesPageScroll()
{
    LayoutNode holder;
    LayoutNode* body = makeBox(-1.0f, -1.0f);
    body->style.overflowY = CSS::Overflow::Hidden;
    LayoutNode* tall = makeBox(-1.0f, 900.0f);
    attach(&holder, body);
    attach(body, tall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(!holder.scrollYEnabled, "body hidden disables page scroll");
    MorphEvent e = wheel(0.0f, -1.0f);
    CHECK(!holder.dispatchEvent(e, 400.0f, 300.0f), "locked page wheel unhandled");
    CHECK(near(holder.scrollY, 0.0f), "locked page holds zero");
}

// A fixed-height box with default (visible) overflow never scrolls
// itself: content paints outside the box, exactly like Chrome.
static void testVisibleDivOverflowsWithoutScrolling()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    LayoutNode* tall = makeBox(-1.0f, 300.0f);
    attach(&holder, box);
    attach(box, tall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(!box->scrollYEnabled, "visible box never arms scroll");
    CHECK(!box->clipsOverflowBox(), "visible box never clips");
    CHECK(tall->y + tall->h > box->y + box->h, "content paints outside the box");
    MorphEvent e = wheel(0.0f, -1.0f);
    holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f);
    CHECK(near(box->scrollY, 0.0f), "visible box holds zero under wheel");
}

// Wheel over a visible box inside a tall page chains past it: the box
// holds still while the page scrolls underneath.
static void testDivWheelChainsToPage()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    LayoutNode* mid = makeBox(-1.0f, 300.0f);
    LayoutNode* tail = makeBox(-1.0f, 700.0f);
    attach(&holder, box);
    attach(box, mid);
    attach(&holder, tail);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    CHECK(holder.scrollYEnabled, "tall page arms root scroll");
    MorphEvent e = wheel(0.0f, -1.0f);
    CHECK(holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f), "chained page wheel handled");
    CHECK(near(box->scrollY, 0.0f), "visible box holds still");
    CHECK(near(holder.scrollY, 40.0f), "page scrolls under visible box");
}

// `scroll-behavior: smooth` arms the glide target on wheel while the
// rendered offset eases toward it; `auto` keeps jumping immediately.
static void testSmoothWheelGlides()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowY = CSS::Overflow::Auto;
    box->style.scrollBehaviorSmooth = true;
    LayoutNode* tall = makeBox(-1.0f, 300.0f);
    attach(&holder, box);
    attach(box, tall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    MorphEvent e = wheel(0.0f, -1.0f);
    CHECK(holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f), "smooth wheel handled");
    CHECK(near(box->scrollTargetY, 40.0f), "smooth wheel arms the target");
    CHECK(near(box->scrollY, 0.0f), "smooth wheel does not jump");
    CHECK(near(e.scroll, 0.0f), "consumed axis zeroes its delta");
    // A second notch accumulates onto the in-flight target.
    MorphEvent e2 = wheel(0.0f, -1.0f);
    holder.dispatchEvent(e2, box->x + 10.0f, box->y + 10.0f);
    CHECK(near(box->scrollTargetY, 80.0f), "notches accumulate on the target");
    // Frames ease toward the target and snap exactly.
    float before = box->scrollY;
    holder.update(1.0f / 60.0f);
    CHECK(box->scrollY > before, "first frame starts gliding");
    CHECK(box->scrollY < box->scrollTargetY, "glide approaches, not jumps");
    for (int i = 0; i < 600; i++) holder.update(1.0f / 60.0f);
    CHECK(near(box->scrollY, 80.0f), "glide snaps to the target");
    CHECK(near(box->scrollTargetY, 80.0f), "target holds after landing");
}

// Without the smooth flag the wheel still jumps one step per notch.
static void testAutoWheelStillJumps()
{
    LayoutNode holder;
    LayoutNode* box = makeBox(200.0f, 100.0f);
    box->style.overflowY = CSS::Overflow::Auto;
    LayoutNode* tall = makeBox(-1.0f, 300.0f);
    attach(&holder, box);
    attach(box, tall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    MorphEvent e = wheel(0.0f, -1.0f);
    holder.dispatchEvent(e, box->x + 10.0f, box->y + 10.0f);
    CHECK(near(box->scrollY, 40.0f), "auto wheel jumps immediately");
    CHECK(near(box->scrollTargetY, 40.0f), "auto wheel keeps target in sync");
}

// A smooth box exhausted to its limit hands the remainder outward.
static void testSmoothWheelChainsAtLimit()
{
    LayoutNode holder;
    LayoutNode* outer = makeBox(300.0f, 200.0f);
    outer->style.overflowY = CSS::Overflow::Auto;
    LayoutNode* inner = makeBox(200.0f, 100.0f);
    inner->style.overflowY = CSS::Overflow::Auto;
    inner->style.scrollBehaviorSmooth = true;
    LayoutNode* tall = makeBox(-1.0f, 400.0f);
    LayoutNode* outerTall = makeBox(-1.0f, 250.0f);
    attach(&holder, outer);
    attach(outer, inner);
    attach(inner, tall);
    attach(outer, outerTall);
    holder.layout(0.0f, 0.0f, 800.0f, 600.0f, nullptr);
    inner->scrollTargetY = inner->contentH - inner->h;
    inner->scrollY = inner->scrollTargetY;
    float px = inner->x + 10.0f, py = inner->y + 10.0f;
    MorphEvent e = wheel(0.0f, -1.0f);
    CHECK(holder.dispatchEvent(e, px, py), "limit wheel handled");
    CHECK(near(outer->scrollY, 40.0f), "spent smooth box chains to outer");
}

int main()
{
    testVerticalOnly();
    testHorizontalOnly();
    testHiddenDisarms();
    testScrollArmsWithoutOverflow();
    testClipDisarms();
    testWheelScrolls();
    testWheelChaining();
    testWheelHorizontal();
    testShiftWheel();
    testClamps();
    testPageScroll();
    testBodyHiddenDisablesPageScroll();
    testVisibleDivOverflowsWithoutScrolling();
    testDivWheelChainsToPage();
    testThumbGeometry();
    testSmoothWheelGlides();
    testAutoWheelStillJumps();
    testSmoothWheelChainsAtLimit();
    std::printf("[scroll-layout-test] %d checks, %d failures\n", checks, failures);
    return failures == 0 ? 0 : 1;
}
