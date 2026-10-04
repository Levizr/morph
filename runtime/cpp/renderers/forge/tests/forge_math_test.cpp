// renderers/forge/tests/forge_math_test.cpp
// Headless unit tests for the pure-CPU forge math: DamageSet union/clip,
// TilePool LRU + epoch + budget, scroll-shift detection + exposed strip.
// No GL context needed (GL symbols link from libGL but are never called).
#include <cstdio>

#include "../damage.h"
#include "../layer.h"
#include "../scroll_shift.h"
#include "../tile_pool.h"

static int g_checks = 0;
static int g_failures = 0;

static void check(bool ok, const char* name)
{
    g_checks++;
    if (!ok)
    {
        g_failures++;
        printf("[forge-math-test] FAIL %s\n", name);
    }
}

static void testDamageRect()
{
    DamageRect a{0, 0, 10, 10};
    DamageRect b{5, 5, 10, 10};
    DamageRect c{20, 20, 5, 5};
    check(a.intersects(b), "rect:overlap");
    check(!a.intersects(c), "rect:disjoint");
    DamageRect i = a.intersection(b);
    check(i.x == 5 && i.y == 5 && i.w == 5 && i.h == 5, "rect:intersection");
    DamageRect e = a.intersection(c);
    check(e.w == 0 && e.h == 0, "rect:empty-intersection");
}

static void testDamageSet()
{
    DamageSet ds;
    check(ds.empty(), "ds:empty");
    ds.add({0, 0, 10, 10});
    ds.add({20, 20, 5, 5});
    check(ds.rects.size() == 2, "ds:disjoint-append");
    ds.add({5, 5, 10, 10});
    check(ds.rects.size() == 2, "ds:overlap-merge");
    check(ds.totalArea() == 15 * 15 + 25, "ds:total-area");
    check(ds.intersects({14, 14, 2, 2}), "ds:intersects-hit");
    check(!ds.intersects({16, 0, 2, 2}), "ds:intersects-miss");

    DamageSet other;
    other.add({100, 100, 10, 10});
    ds.merge(other);
    check(ds.rects.size() == 3, "ds:merge-append");

    DamageSet full;
    full.setFullScreen();
    check(!full.empty(), "ds:fullscreen-not-empty");
    check(full.intersects({9999, 9999, 1, 1}), "ds:fullscreen-intersects-all");
    full.add({0, 0, 4, 4});
    check(full.rects.empty(), "ds:add-ignored-when-fullscreen");
    ds.merge(full);
    check(ds.fullScreen, "ds:merge-fullscreen");

    DamageSet clip;
    clip.add({-5, -5, 20, 20});
    clip.add({10000, 10000, 4, 4});
    clip.clipTo(800, 600);
    check(clip.rects.size() == 1, "ds:clip-drops-outside");
    check(clip.rects[0].x == 0 && clip.rects[0].y == 0, "ds:clip-origin");
    check(clip.rects[0].w == 15 && clip.rects[0].h == 15, "ds:clip-size");
}

static void testTilePool()
{
    TilePool pool(1000);
    TileKey a{0, 0, 0, 10, 10};
    TileKey b{0, 20, 20, 10, 10};
    TileKey c{0, 40, 40, 20, 10};
    check(tileBytes(a) == 400, "tile:bytes");
    uint64_t epoch0 = pool.epoch();
    pool.acquire(a);
    pool.acquire(b);
    check(pool.tileCount() == 2, "tile:two-resident");
    check(pool.bytesUsed() == 800, "tile:bytes-used");
    check(pool.contains(a) && pool.contains(b), "tile:contains");
    check(pool.epoch() == epoch0, "tile:acquire-keeps-epoch");
    pool.acquire(c);
    check(pool.tileCount() == 1, "tile:budget-evicts-lru");
    check(pool.contains(c), "tile:newest-survives");
    check(!pool.contains(a), "tile:oldest-evicted");
    check(pool.bytesUsed() == 800, "tile:bytes-after-evict");

    TilePool pool2(100000);
    TileKey d{0, 0, 0, 10, 10};
    TileKey e{0, 50, 50, 10, 10};
    uint64_t epoch1 = pool2.epoch();
    pool2.acquire(d);
    pool2.acquire(e);
    check(pool2.tileCount() == 2, "tile:overlap-coexist");
    pool2.invalidateOverlapping(0, 0, 6, 6);
    check(!pool2.contains(d), "tile:invalidate-overlap-hit");
    check(pool2.contains(e), "tile:invalidate-overlap-miss-kept");
    check(pool2.epoch() > epoch1, "tile:invalidate-bumps-epoch");
    pool2.clear();
    check(pool2.tileCount() == 0 && pool2.bytesUsed() == 0, "tile:clear");
    TileKey zero{0, 0, 0, 0, 10};
    check(pool.acquire(zero) == 0, "tile:zero-size-rejected");
}

static void testScrollShift()
{
    forge::ScrollShift shift;
    check(
        forge::detectScrollShift(0, 0, 800, 1000, 0.0f, 100.0f, 3000.0f, false, false, shift),
        "shift:scroll-down");
    check(shift.m_deltaY == 100, "shift:delta");
    DamageRect strip = forge::exposedStrip(shift);
    check(strip.x == 0 && strip.y == 900 && strip.w == 800 && strip.h == 100,
          "shift:exposed-bottom");

    check(
        forge::detectScrollShift(0, 0, 800, 1000, 100.0f, 50.0f, 3000.0f, false, false, shift),
        "shift:scroll-up");
    strip = forge::exposedStrip(shift);
    check(strip.x == 0 && strip.y == 0 && strip.w == 800 && strip.h == 50,
          "shift:exposed-top");

    check(!forge::detectScrollShift(0, 0, 800, 1000, 50.0f, 50.0f, 3000.0f, false, false, shift),
          "shift:no-delta");
    check(!forge::detectScrollShift(0, 0, 800, 1000, 0.0f, 100.0f, 3000.0f, true, false, shift),
          "shift:transformed-fallback");
    check(!forge::detectScrollShift(0, 0, 800, 1000, 0.0f, 100.0f, 3000.0f, false, true, shift),
          "shift:rounded-fallback");
    check(!forge::detectScrollShift(0, 0, 800, 1000, 0.0f, 1200.0f, 3000.0f, false, false, shift),
          "shift:delta-exceeds-height");
    check(!forge::detectScrollShift(0, 0, 800, 1000, 0.0f, 100.0f, 800.0f, false, false, shift),
          "shift:content-fits");
}

static void testMoverBox()
{
    DamageRect still = moverBox({0, 0, 100, 50}, 0.0f, 0.0f);
    check(still.x == -2 && still.y == -2 && still.w == 104 && still.h == 54,
          "mover:idle-margin");
    DamageRect moved = moverBox({10, 20, 30, 40}, 5.5f, -3.2f);
    check(moved.x == 13 && moved.y == 15 && moved.w == 34 && moved.h == 44,
          "mover:shifted-margin");
    DamageRect spanStill = moverSpan({0, 0, 100, 50}, 0.0f, 0.0f);
    check(spanStill.x == -2 && spanStill.y == -2 && spanStill.w == 104 &&
              spanStill.h == 54,
          "mover-span:idle");
    DamageRect span = moverSpan({20, 20, 100, 40}, 60.0f, 0.0f);
    check(span.x == 18 && span.y == 18 && span.w == 164 && span.h == 44,
          "mover-span:right");
    DamageRect spanUp = moverSpan({20, 100, 100, 40}, 0.0f, -30.0f);
    check(spanUp.x == 18 && spanUp.y == 68 && spanUp.w == 104 && spanUp.h == 74,
          "mover-span:up");
}

static forge::MoverLayerGates eligibleGates()
{
    forge::MoverLayerGates g;
    g.m_w = 100;
    g.m_h = 50;
    g.m_hasOffset = true;
    g.m_animOpacityOne = true;
    g.m_opacityOne = true;
    g.m_isLeaf = true;
    g.m_hasText = false;
    g.m_hasRadius = false;
    g.m_hasBorder = false;
    g.m_overflowVisible = true;
    g.m_scrollEnabled = false;
    g.m_hasLayoutTransition = false;
    g.m_transformed = false;
    g.m_singleRectOpaque = true;
    g.m_colorAnim = false;
    g.m_opacityAnim = false;
    g.m_radiusAnim = false;
    g.m_ancestorTransformed = false;
    g.m_ancestorRounded = false;
    g.m_ancestorScrolled = false;
    g.m_ancestorClipped = false;
    g.m_overlapsSibling = false;
    g.m_multipleMovers = false;
    return g;
}

static void testLayerPool()
{
    forge::LayerPool layers;
    check(!layers.shouldPromote(1, 100, 100, false, false), "layer:static-no-promote");
    check(layers.shouldPromote(1, 100, 100, true, false), "layer:transition-promote");
    check(layers.shouldPromote(2, 100, 100, false, true), "layer:color-anim-promote");
    check(layers.shouldPromote(4, 100, 100, false, false, true), "layer:position-promote");
    check(!layers.shouldPromote(3, 900, 900, true, false), "layer:oversize-no-promote");
    layers.update(1, 100, 100, 7);
    check(layers.contains(1), "layer:tracked");
    check(layers.layerCount() == 1, "layer:count");
    layers.prune({1}, 8);
    check(layers.contains(1), "layer:live-kept");
    layers.prune({}, 200);
    check(!layers.contains(1), "layer:stale-pruned");

    check(forge::isMoverLayerEligible(eligibleGates()), "mover-layer:eligible");
    forge::MoverLayerGates g = eligibleGates();
    g.m_hasOffset = false;
    check(!forge::isMoverLayerEligible(g), "mover-layer:no-offset");
    g = eligibleGates();
    g.m_hasText = true;
    check(!forge::isMoverLayerEligible(g), "mover-layer:text");
    g = eligibleGates();
    g.m_hasRadius = true;
    check(!forge::isMoverLayerEligible(g), "mover-layer:radius");
    g = eligibleGates();
    g.m_colorAnim = true;
    check(!forge::isMoverLayerEligible(g), "mover-layer:color-anim");
    g = eligibleGates();
    g.m_overlapsSibling = true;
    check(!forge::isMoverLayerEligible(g), "mover-layer:overlap");
    g = eligibleGates();
    g.m_multipleMovers = true;
    check(!forge::isMoverLayerEligible(g), "mover-layer:multi");
    g = eligibleGates();
    g.m_ancestorScrolled = true;
    check(!forge::isMoverLayerEligible(g), "mover-layer:ancestor-scroll");
    g = eligibleGates();
    g.m_w = 600;
    check(!forge::isMoverLayerEligible(g), "mover-layer:oversize");
    g = eligibleGates();
    g.m_singleRectOpaque = false;
    check(!forge::isMoverLayerEligible(g), "mover-layer:non-rect");
}

int main()
{
    testDamageRect();
    testDamageSet();
    testTilePool();
    testScrollShift();
    testMoverBox();
    testLayerPool();
    printf("[forge-math-test] %d checks, %d failures\n", g_checks, g_failures);
    return g_failures == 0 ? 0 : 1;
}
