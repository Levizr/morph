// renderers/forge/forge.cpp
#include "forge/forge.h"
#include "forge/damage.h"
#include "../core/render_frame.h"
#include "../core/window.h"
#include "../render/gl_renderer.h"
#include <chrono>
#include <cstdio>
#include <functional>
#include <unordered_map>
#include <unordered_set>

namespace forge {

// Phase 3 (retained surface): a persistent FBO + color texture holds the last
// fully-presented frame. Each present clears/redraws/blits only the damaged
// regions accumulated during commit. Idle and tiny-change frames therefore do
// a fraction of the raster/present work of the full flash pass.
//
// Correctness rules baked into the damage policy:
//  * Compositor-geometry animations (X/Y) move pixels *after* commit, on the
//    compositor thread, so commit-time damage can't recover their old rects —
//    force a fullscreen repaint while any is running.
//  * Color/radius compositor animations keep a stable box, so the node's rect
//    is added explicitly (its pixels change every interpolated frame).
//  * Main-thread transitions (hover/active) mark nodes PaintDirty every frame,
//    so addAll() picks them up; a live prev-rect map adds each moved node's
//    OLD position as damage so the retained surface doesn't leave ghosts.
//  * Dangling prev-rect entries are never dereferenced (only compared against
//    live pointers) and the map is pruned on fullscreen / tree rebuild, so a
//    freed node address can only yield a harmless extra repaint.

// Damage from the last commit (consumed by present, refreshed by the next one).
static DamageSet g_damage;

// ── Retained surface ─────────────────────────────────────────────
static GLuint g_fbo = 0, g_fboTex = 0, g_fboRbo = 0;
static int g_fboW = 0, g_fboH = 0;
static bool g_surfaceReady = false;

// Live-node geometry from the last commit (old-position damage recovery).
// Tracks scroll too so a changed scrollport is detected as a box move.
struct PrevRect {
    DamageRect box;
    float scrollY = 0;
    float contentH = 0;
};
static std::unordered_map<const MorphNode*, PrevRect> g_prevRects;
static int g_prevNodeCount = -1;
static bool g_firstFrame = true;

static void destroySurface()
{
    if (g_fboTex) { glDeleteTextures(1, &g_fboTex); g_fboTex = 0; }
    if (g_fboRbo) { glDeleteRenderbuffers(1, &g_fboRbo); g_fboRbo = 0; }
    if (g_fbo) { glDeleteFramebuffers(1, &g_fbo); g_fbo = 0; }
    g_fboW = g_fboH = 0;
    g_surfaceReady = false;
}

// Return true if the surface was just (re)created (caller must repaint fully).
static bool ensureSurface(int w, int h)
{
    if (g_fbo && g_fboW == w && g_fboH == h)
        return false;
    destroySurface();

    glGenFramebuffers(1, &g_fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, g_fbo);

    glGenTextures(1, &g_fboTex);
    glBindTexture(GL_TEXTURE_2D, g_fboTex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, g_fboTex, 0);

    // Depth + stencil so scissored clears and rounded-clip masks work in-FBO.
    glGenRenderbuffers(1, &g_fboRbo);
    glBindRenderbuffer(GL_RENDERBUFFER, g_fboRbo);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, w, h);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT,
                              GL_RENDERBUFFER, g_fboRbo);

    GLenum status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
    g_surfaceReady = (status == GL_FRAMEBUFFER_COMPLETE);
    if (!g_surfaceReady)
        fprintf(stderr, "[forge] retained FBO incomplete (0x%x)\n", status);

    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    g_fboW = w; g_fboH = h;
    return true;
}

// ── Damage accumulation helpers ──────────────────────────────────
static void walkTree(MorphNode* n, const std::function<void(MorphNode*)>& fn)
{
    fn(n);
    for (auto* c : n->children)
        walkTree(c, fn);
}

static bool hasGeometryCompositorAnim(MorphNode* n)
{
    for (auto& a : n->m_animations)
        if (a.running && !a.finished &&
            (a.property == AnimProperty::X || a.property == AnimProperty::Y))
            return true;
    return false;
}

void forgeCommit(MorphWindow& win)
{
    if (!win.hasRoot())
        return;

    // Phase 1-2: Layout + paint (GL context is already current on main thread)
    win.renderer().ensureReady();

    auto& stats = win.dirtyStats();
    stats.reset();

    // Snapshot genuine paint dirt BEFORE layout: production layoutIfNeeded()
    // blanket-marks every re-laid node PaintDirty, which would otherwise widen
    // damage to the whole reflowed region. These pre-layout marks are the real
    // "content changed but the box didn't" signal (setText at a stable width,
    // hover/active styles, etc.).
    std::unordered_set<MorphNode*> paintBefore;
    walkTree(win.root(), [&](MorphNode* n) {
        if (n->isDirty(PaintDirty))
            paintBefore.insert(n);
    });

    win.root()->layoutIfNeeded(0.0f, 0.0f, win.contentWidth(), win.contentHeight(),
                               &win.renderer(), &stats);
    stats.fullTreeCount = countNodes(win.root());

#ifdef MORPH_FEATURE_DEV
    syncPaintDirtyTree(win.root());
#endif

    // Phase 3: Accumulate damage BEFORE recordPaintTree clears the dirty flags
    // that the sweep below inspects.
    int vw = win.width(), vh = win.height();
    int nodeCount = stats.fullTreeCount;
    DamageSet damage;
    bool geometryAnim = false;

    walkTree(win.root(), [&](MorphNode* n) {
        if (hasGeometryCompositorAnim(n))
            geometryAnim = true;
        // Non-geometry compositor anims repaint in place each interpolated
        // frame — their committed box is stable, so schedule the node's rect.
        for (auto& a : n->m_animations)
        {
            if (a.running && !a.finished &&
                a.property != AnimProperty::X && a.property != AnimProperty::Y)
                damage.add({(int)n->x, (int)n->y, (int)n->w, (int)n->h});
        }
    });

    if (g_firstFrame || geometryAnim || nodeCount != g_prevNodeCount)
    {
        damage.setFullScreen();
        g_prevRects.clear();
    }
    else
    {
        // Mirrors the dev-mode geometry diff: repaint a node iff its box moved
        // (old + new) or it was genuinely paint-dirty before layout. Drop the
        // blanket PaintDirty production layout() stamps on re-laid nodes whose
        // pixels are actually unchanged, so a local change stays local instead
        // of widening damage to the whole reflowed region.
        walkTree(win.root(), [&](MorphNode* n) {
            auto it = g_prevRects.find(n);
            bool boxChanged = it == g_prevRects.end();
            if (!boxChanged)
            {
                const PrevRect& prev = it->second;
                boxChanged = ((int)n->x != prev.box.x || (int)n->y != prev.box.y ||
                              (int)n->w != prev.box.w || (int)n->h != prev.box.h ||
                              (int)n->scrollY != (int)prev.scrollY ||
                              (int)n->contentH != (int)prev.contentH);
            }
            if (boxChanged)
            {
                if (it != g_prevRects.end())
                    damage.add(it->second.box); // old position
                damage.add({(int)n->x, (int)n->y, (int)n->w, (int)n->h});
                n->markDirty(PaintDirty);       // force display-list re-record
            }
            else if (paintBefore.count(n))
            {
                damage.add({(int)n->x, (int)n->y, (int)n->w, (int)n->h});
            }
            else
            {
                n->clearDirty(PaintDirty);
                n->clearDirty(ScrollDirty);
            }
        });
        // 1px safety margin past the rounded clip boundary, then clip to view.
        for (auto& r : damage.rects)
        {
            r.x -= 1; r.y -= 1;
            r.w += 2; r.h += 2;
        }
        damage.clipTo(vw, vh);
    }

    // Prune stale prev-rect entries (trees rebuilt on dev hot reload).
    if ((int)g_prevRects.size() > nodeCount * 2 + 32)
        g_prevRects.clear();

    g_firstFrame = false;
    g_prevNodeCount = nodeCount;
    walkTree(win.root(), [&](MorphNode* n) {
        g_prevRects[n] = {{(int)n->x, (int)n->y, (int)n->w, (int)n->h},
                          n->scrollY, n->contentH};
    });

    stats.damageArea = damage.totalArea();
    g_damage = std::move(damage);

    recordPaintTree(win.root(), win.renderer(), stats);

    // Phase 3: Flatten into render frame (no GL needed)
    int backIdx = g_backIndex.load();
    RenderFrame &frame = g_backFrames[backIdx];
    frame.nodes.clear();
    frame.drawOps.clear();
    frame.animations.clear();
    frame.textOps.clear();
    frame.frameId++;
    auto now = std::chrono::steady_clock::now().time_since_epoch();
    frame.timestamp = std::chrono::duration<double>(now).count();

    win.root()->flatten(frame, -1);

    // Phase 4: Atomic swap — compositor interpolates, then main thread presents
    g_frontFrame.store(&frame, std::memory_order_release);
    g_backIndex.store((backIdx + 1) % 2, std::memory_order_release);
    g_framePending.store(true, std::memory_order_release);

    win.clearPendingRender();
}

void forgePresent(MorphWindow& win,
                  std::function<void(GLRenderer&, DirtyStats&)> overlayFn)
{
    if (!win.handle())
        return;

    // Wait for the compositor's interpolated frame.
    while (!g_frameInterpolated.load(std::memory_order_acquire))
        std::this_thread::yield();
    g_frameInterpolated.store(false, std::memory_order_release);

    RenderFrame* frame = g_frontFrame.load(std::memory_order_acquire);
    if (!frame)
        return;

    int w = win.width(), h = win.height();
    GLRenderer& r = win.renderer();

    float proj[16];
    {
        proj[0] = 2.0f / (float)w;   proj[4] = 0; proj[8] = 0;   proj[12] = -1.0f;
        proj[1] = 0;                  proj[5] = -2.0f / (float)h; proj[9] = 0; proj[13] = 1.0f;
        proj[2] = 0;                  proj[6] = 0; proj[10] = 1.0f;  proj[14] = 0;
        proj[3] = 0;                  proj[7] = 0; proj[11] = 0;     proj[15] = 1.0f;
    }

    bool fresh = ensureSurface(w, h);

    auto& stats = win.dirtyStats();
    bool fullscreen = g_damage.fullScreen || fresh;

    if (fresh)
    {
        // Newly created surface is empty — rebuild it entirely.
        g_firstFrame = true;
        fullscreen = true;
    }

    if (!fullscreen && g_damage.empty())
    {
        // Nothing visually changed: the retained surface and the (now-empty
        // after swap) backbuffer both need the full frame pushed through.
        // Blit the whole FBO and swap so a pending commit is never dropped.
        glBindFramebuffer(GL_READ_FRAMEBUFFER, g_fbo);
        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
        glReadBuffer(GL_COLOR_ATTACHMENT0);
        glDrawBuffer(GL_BACK);
        glBlitFramebuffer(0, 0, w, h, 0, 0, w, h, GL_COLOR_BUFFER_BIT, GL_NEAREST);
        glBindFramebuffer(GL_READ_FRAMEBUFFER, 0);
        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);

        if (overlayFn)
        {
            overlayFn(r, stats);
            r.flush(proj);
        }
        glfwSwapBuffers(win.handle());
        return;
    }

    r.setFBHeight(h);
    glBindFramebuffer(GL_FRAMEBUFFER, g_fbo);
    glViewport(0, 0, w, h);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

    if (fullscreen)
    {
        r.setClearColor(1.0f, 1.0f, 1.0f, 1.0f);
        r.clear();
        r.setProjection(proj);
        win.drawFrameNodes();
        stats.damageArea = w * h;
    }
    else
    {
        // Reset depth+stencil for the WHOLE surface (cheap, no color write) so
        // rounded-clip masks start from 0 every frame regardless of region.
        glColorMask(GL_FALSE, GL_FALSE, GL_FALSE, GL_FALSE);
        glClear(GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
        glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);

        // Color-clear each damage rect individually (the rects are disjoint,
        // so no pixel outside them is erased — anything outside gets retained).
        r.setClearColor(1.0f, 1.0f, 1.0f, 1.0f);
        glEnable(GL_SCISSOR_TEST);
        for (const auto& dmg : g_damage.rects)
        {
            glScissor(dmg.x, h - (dmg.y + dmg.h), dmg.w, dmg.h);
            glClear(GL_COLOR_BUFFER_BIT);
        }
        glDisable(GL_SCISSOR_TEST);

        // Re-raster only nodes that touch the damage; everything else is
        // retained in the surface as-is.
        r.setProjection(proj);
        win.drawFrameNodes(&g_damage);

        stats.damageArea = g_damage.totalArea();
    }

    glBindFramebuffer(GL_FRAMEBUFFER, 0);

    // The swapchain back buffer is UNDEFINED after each swap, so we can never
    // blit only a sub-rect — present the whole retained surface every frame.
    // The savings come from the retained re-raster above, not from the blit.
    glBindFramebuffer(GL_READ_FRAMEBUFFER, g_fbo);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
    glReadBuffer(GL_COLOR_ATTACHMENT0);
    glDrawBuffer(GL_BACK);
    glBlitFramebuffer(0, 0, w, h, 0, 0, w, h, GL_COLOR_BUFFER_BIT, GL_NEAREST);
    glBindFramebuffer(GL_READ_FRAMEBUFFER, 0);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);

    stats.presentBytes = w * h * 4;

    if (overlayFn)
    {
        overlayFn(r, stats);
        r.flush(proj);
    }

    glfwSwapBuffers(win.handle());
}

} // namespace forge