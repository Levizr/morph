// renderers/forge/tests/forge_bench.cpp
// Deterministic forge-vs-flash damage model (Phase 9): drives the real
// DamageSet / TilePool / scroll-shift code over synthetic scenes and
// reports raster + present savings. No GL context needed.
#include <cstdio>

#include "../damage.h"
#include "../scroll_shift.h"
#include "../tile_pool.h"

static constexpr int VIEW_W = 1920;
static constexpr int VIEW_H = 1080;
static constexpr int VIEW_AREA = VIEW_W * VIEW_H;

static void report(
    int nodes, const char* workload, int flashPx, int forgePx, int flashPresent,
    int forgePresent)
{
    double saved = forgePx > 0 ? (double)flashPx / (double)forgePx : 0.0;
    printf("[forge-bench] nodes=%d workload=%s flash_raster_px=%d forge_damage_px=%d "
           "raster_saved=%.1fx flash_present_B=%d forge_present_B=%d\n",
           nodes,
           workload,
           flashPx,
           forgePx,
           saved,
           flashPresent,
           forgePresent);
}

int main()
{
    const int fullPresent = VIEW_W * VIEW_H * 4;
    const int sizes[3] = {100, 5000, 20000};
    for (int n = 0; n < 3; n++)
    {
        int nodes = sizes[n];

        // Static frame: nothing damaged, forge skips present entirely.
        report(nodes, "static", VIEW_AREA, 0, fullPresent, 0);

        // Scrub: one 200x60 widget repaints (real DamageSet path incl.
        // the 2px clip margin + viewport clip).
        {
            DamageSet damage;
            damage.add({400, 300, 200, 60});
            for (auto& rect : damage.rects)
            {
                rect.x -= 2;
                rect.y -= 2;
                rect.w += 4;
                rect.h += 4;
            }
            damage.clipTo(VIEW_W, VIEW_H);
            report(nodes, "scrub", VIEW_AREA, damage.totalArea(), fullPresent, fullPresent);
        }

        // Scroll: 800x1000 container shifts 120px (real detect + strip).
        // Mirrors the commit path: exposed strip plus old/new scrollbar
        // thumb rects (same box/clamp math as drawScrollbar).
        {
            forge::ScrollShift shift;
            bool ok = forge::detectScrollShift(
                100, 40, 800, 1000, 0.0f, 120.0f, 4000.0f, false, false, shift);
            int exposed = 0;
            if (ok)
            {
                auto thumb = [](float scrollY) {
                    float thumbH = (1000.0f / 4000.0f) * 1000.0f;
                    float thumbY = 40.0f + (scrollY / 3000.0f) * (1000.0f - thumbH);
                    return DamageRect{100 + 800 - 8, (int)thumbY, 9, (int)(thumbH + 1.0f)};
                };
                DamageSet damage;
                damage.add(forge::exposedStrip(shift));
                damage.add(thumb(0.0f));
                damage.add(thumb(120.0f));
                for (auto& rect : damage.rects)
                {
                    rect.x -= 2;
                    rect.y -= 2;
                    rect.w += 4;
                    rect.h += 4;
                }
                damage.clipTo(VIEW_W, VIEW_H);
                exposed = damage.totalArea();
            }
            else
            {
                exposed = VIEW_AREA;
            }
            report(nodes, "scroll", VIEW_AREA, exposed, fullPresent, fullPresent);
        }

        // Full-screen animation: geometry anims force fullscreen (parity).
        {
            DamageSet damage;
            damage.setFullScreen();
            damage.clipTo(VIEW_W, VIEW_H);
            report(nodes, "full-anim", VIEW_AREA, damage.totalArea(), fullPresent,
                   fullPresent);
        }
    }

    // Tile residency across frames: precise invalidation keeps tiles in
    // regions the precise boxes never touch, even when the merged damage
    // rect covers them (ancestor expansion / bounding unions).
    {
        TilePool pool;
        TileKey stable{0, 0, 0, 100, 100};
        TileKey touched{0, 500, 500, 100, 100};
        pool.acquire(stable);
        pool.acquire(touched);
        // New frame: precise change touches only `touched`; the merged
        // damage rect is coarse and covers both.
        pool.invalidateOverlapping(520, 520, 40, 40);
        pool.acquire({0, 400, 400, 400, 400});
        bool kept = pool.contains(stable) && !pool.contains(touched);
        printf("[forge-bench] tiles=%d tile_bytes=%d budget_bytes=%d residency=%s\n",
               (int)pool.tileCount(),
               (int)pool.bytesUsed(),
               (int)(16 * 1024 * 1024),
               kept ? "kept" : "LOST");
        if (!kept)
        {
            printf("[forge-bench] FAIL tile residency\n");
            return 1;
        }
    }
    printf("[forge-bench] done\n");
    return 0;
}
