// renderers/forge/forge.cpp
#include "forge/forge.h"

#include "forge/damage.h"
#include "forge/layer.h"
#include "forge/scroll_shift.h"
#include "forge/tile_pool.h"
#include "../core/render_frame.h"
#include "../core/window.h"
#include "../render/gl_renderer.h"

#include <chrono>
#include <cmath>
#include <cstdio>
#include <functional>
#include <thread>
#include <unordered_map>
#include <unordered_set>
#include <vector>

namespace forge
{

struct PrevRect
{
    DamageRect m_box;
    float m_scrollX = 0;
    float m_scrollY = 0;
    float m_contentW = 0;
    float m_contentH = 0;
};

struct ForgeWindowState
{
    DamageSet m_damage;
    TilePool m_tiles;
    LayerPool m_layers;
    std::unordered_map<const MorphNode*, PrevRect> m_prevRects;
    std::vector<ScrollShift> m_shifts;
    // Precise changed boxes (pre-merge, screen space): tile invalidation
    // follows these, while clear + raster follow the merged damage set.
    std::vector<DamageRect> m_precise;
    int m_prevNodeCount = -1;
    bool m_firstFrame = true;
    uint64_t m_forceGenSeen = 0;
    uint64_t m_frameId = 0;
    GLuint m_fbo = 0;
    GLuint m_fboTex = 0;
    GLuint m_fboRbo = 0;
    int m_fboW = 0;
    int m_fboH = 0;
    bool m_surfaceReady = false;
    // Scroll-shift scratch surface (grow-only per window, freed with the
    // window): stages shifted tile content so overlapping copies never
    // read their own output. Sized to the largest shift seen.
    GLuint m_shiftFbo = 0;
    GLuint m_shiftTex = 0;
    int m_shiftW = 0;
    int m_shiftH = 0;
    // Saved mover-layer key (negative frame index, see tryMoverLayer) and
    // the frame index it was captured for. Lets commit preserve the
    // present-side layer across frames while its position anim runs;
    // cleared on fullscreen like everything else.
    int m_moverKey = 0;
    int m_moverIdx = -1;
    bool m_moverHave = false;
};

static std::unordered_map<MorphWindow*, ForgeWindowState> g_states;
static uint64_t g_forceGen = 0;
static constexpr int MAX_ANIMS_BEFORE_FULLSCREEN = 32;

void forceFullscreen()
{
    g_forceGen++;
}

void forgetWindow(MorphWindow& win)
{
    auto it = g_states.find(&win);
    if (it == g_states.end())
    {
        return;
    }
    ForgeWindowState& state = it->second;
    if (state.m_fboTex != 0)
    {
        glDeleteTextures(1, &state.m_fboTex);
    }
    if (state.m_fboRbo != 0)
    {
        glDeleteRenderbuffers(1, &state.m_fboRbo);
    }
    if (state.m_fbo != 0)
    {
        glDeleteFramebuffers(1, &state.m_fbo);
    }
    if (state.m_shiftTex != 0)
    {
        glDeleteTextures(1, &state.m_shiftTex);
    }
    if (state.m_shiftFbo != 0)
    {
        glDeleteFramebuffers(1, &state.m_shiftFbo);
    }
    state.m_layers.clear();
    g_states.erase(it);
}

static void destroySurface(ForgeWindowState& state)
{
    if (state.m_fboTex != 0)
    {
        glDeleteTextures(1, &state.m_fboTex);
        state.m_fboTex = 0;
    }
    if (state.m_fboRbo != 0)
    {
        glDeleteRenderbuffers(1, &state.m_fboRbo);
        state.m_fboRbo = 0;
    }
    if (state.m_fbo != 0)
    {
        glDeleteFramebuffers(1, &state.m_fbo);
        state.m_fbo = 0;
    }
    if (state.m_shiftTex != 0)
    {
        glDeleteTextures(1, &state.m_shiftTex);
        state.m_shiftTex = 0;
    }
    if (state.m_shiftFbo != 0)
    {
        glDeleteFramebuffers(1, &state.m_shiftFbo);
        state.m_shiftFbo = 0;
    }
    state.m_shiftW = 0;
    state.m_shiftH = 0;
    state.m_fboW = 0;
    state.m_fboH = 0;
    state.m_surfaceReady = false;
}

// Grow-only scratch FBO for scroll-shift staging (see applyScrollShift).
// Returns 0 when no GL surface is available yet.
static GLuint ensureShiftScratch(ForgeWindowState& state, int w, int h)
{
    if (w <= 0 || h <= 0)
    {
        return 0;
    }
    if (state.m_shiftFbo != 0 && state.m_shiftW >= w && state.m_shiftH >= h)
    {
        return state.m_shiftFbo;
    }
    if (state.m_shiftTex != 0)
    {
        glDeleteTextures(1, &state.m_shiftTex);
        state.m_shiftTex = 0;
    }
    if (state.m_shiftFbo != 0)
    {
        glDeleteFramebuffers(1, &state.m_shiftFbo);
        state.m_shiftFbo = 0;
    }
    glGenFramebuffers(1, &state.m_shiftFbo);
    glBindFramebuffer(GL_FRAMEBUFFER, state.m_shiftFbo);
    glGenTextures(1, &state.m_shiftTex);
    glBindTexture(GL_TEXTURE_2D, state.m_shiftTex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D,
                           state.m_shiftTex, 0);
    bool ok = glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE;
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    if (!ok)
    {
        glDeleteTextures(1, &state.m_shiftTex);
        glDeleteFramebuffers(1, &state.m_shiftFbo);
        state.m_shiftTex = 0;
        state.m_shiftFbo = 0;
        return 0;
    }
    state.m_shiftW = w;
    state.m_shiftH = h;
    return state.m_shiftFbo;
}

static bool ensureSurface(ForgeWindowState& state, int w, int h)
{
    if (state.m_fbo != 0 && state.m_fboW == w && state.m_fboH == h)
    {
        return false;
    }
    destroySurface(state);

    glGenFramebuffers(1, &state.m_fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, state.m_fbo);

    glGenTextures(1, &state.m_fboTex);
    glBindTexture(GL_TEXTURE_2D, state.m_fboTex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, state.m_fboTex, 0);

    glGenRenderbuffers(1, &state.m_fboRbo);
    glBindRenderbuffer(GL_RENDERBUFFER, state.m_fboRbo);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, w, h);
    glFramebufferRenderbuffer(
        GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT, GL_RENDERBUFFER, state.m_fboRbo);

    GLenum status = glCheckFramebufferStatus(GL_FRAMEBUFFER);
    state.m_surfaceReady = (status == GL_FRAMEBUFFER_COMPLETE);
    if (!state.m_surfaceReady)
    {
        fprintf(stderr, "[forge] retained FBO incomplete (0x%x)\n", status);
    }

    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    state.m_fboW = w;
    state.m_fboH = h;
    return true;
}

static void walkTree(MorphNode* node, const std::function<void(MorphNode*)>& fn)
{
    fn(node);
    for (auto* child : node->children)
    {
        walkTree(child, fn);
    }
}

static bool hasColorCompositorAnim(MorphNode* node)
{
    for (auto& anim : node->m_animations)
    {
        if (anim.running && !anim.finished &&
            anim.property != AnimProperty::X && anim.property != AnimProperty::Y)
        {
            return true;
        }
    }
    return false;
}

static bool hasPositionCompositorAnim(MorphNode* node)
{
    for (auto& anim : node->m_animations)
    {
        if (anim.running && !anim.finished &&
            (anim.property == AnimProperty::X || anim.property == AnimProperty::Y))
        {
            return true;
        }
    }
    return false;
}

// Scrollbar thumb rect for a scroll offset. Mirrors the draw math in
// MorphWindow::drawScrollbar (same box, same clamp) so the damage always
// covers the drawn thumb; the global 1px margin absorbs float truncation.
static DamageRect thumbRect(int x, int y, int w, int h, float scrollY, float contentH, float barW)
{
    float boxH = (float)h;
    float thumbH = (boxH / contentH) * boxH;
    float denom = contentH - boxH;
    float thumbY = (float)y;
    if (denom > 0.0f)
    {
        thumbY = (float)y + (scrollY / denom) * (boxH - thumbH);
    }
    if (thumbY < (float)y)
    {
        thumbY = (float)y;
    }
    if (thumbY + thumbH > (float)y + boxH)
    {
        thumbY = (float)y + boxH - thumbH;
    }
    int bar = (int)barW;
    return {x + w - bar, (int)thumbY, bar + 1, (int)(thumbH + 1.0f)};
}

// Screen-space X/Y of a node: root-space minus every scrolling ancestor's
// offset. This is the same rule renderNode uses when drawing (an axis
// scrolls only when its content overflows), so commit-time damage and
// present-time culling share one coordinate space. Without it, partial
// damage over scrolled content both clears and culls the wrong region.
static float screenXOf(MorphNode* node)
{
    float sx = node->x;
    for (MorphNode* p = node->parent; p; p = p->parent)
    {
        if (p->scrollXEnabled && p->contentW > p->w)
        {
            sx -= p->scrollX;
        }
    }
    return sx;
}

static float screenYOf(MorphNode* node)
{
    float sy = node->y;
    for (MorphNode* p = node->parent; p; p = p->parent)
    {
        if (p->scrollYEnabled && p->contentH > p->h)
        {
            sy -= p->scrollY;
        }
    }
    return sy;
}

// Screen-space box of a node (no ancestor expansion).
static DamageRect nodeScreenBox(MorphNode* node)
{
    return {static_cast<int>(screenXOf(node)), static_cast<int>(screenYOf(node)),
            static_cast<int>(node->w), static_cast<int>(node->h)};
}

// Damage a node in screen space: its own screen box plus the screen boxes
// of scroll ancestors whose clip region it can affect (mirrors
// DamageSet::add(MorphNode*) one space over).
static void damageNodeScreen(DamageSet& damage, MorphNode* node)
{
    damage.add(nodeScreenBox(node));
    for (MorphNode* a = node->parent; a; a = a->parent)
    {
        if (a->scrollXEnabled || a->scrollYEnabled)
        {
            damage.add({static_cast<int>(screenXOf(a)), static_cast<int>(screenYOf(a)),
                        static_cast<int>(a->w), static_cast<int>(a->h)});
        }
    }
}

// True when a scroll-shift of node is provably safe: no ancestor remaps
// pixels with a transform, cuts them with a rounded clip or a
// non-containing rectangular clip, and no ancestor adds its own scroll
// offset (nested shifts fall back to full-container damage).
static bool shiftSafeAncestors(MorphNode* node)
{
    int contX = static_cast<int>(screenXOf(node));
    int contY = static_cast<int>(screenYOf(node));
    int contR = contX + static_cast<int>(node->w);
    int contB = contY + static_cast<int>(node->h);
    for (MorphNode* p = node->parent; p; p = p->parent)
    {
        if (!p->style.borderRadius.isZero())
        {
            return false;
        }
#ifdef MORPH_FEATURE_TRANSFORM
        if (p->style.transformSet)
        {
            return false;
        }
#endif
        if (p->scrollYEnabled && p->contentH > p->h && p->scrollY != 0.0f)
        {
            return false;
        }
        if (p->scrollXEnabled && p->contentW > p->w && p->scrollX != 0.0f)
        {
            return false;
        }
        if (p->style.overflowX != CSS::Overflow::Visible ||
            p->style.overflowY != CSS::Overflow::Visible)
        {
            int ancX = static_cast<int>(screenXOf(p));
            int ancY = static_cast<int>(screenYOf(p));
            int ancR = ancX + static_cast<int>(p->w);
            int ancB = ancY + static_cast<int>(p->h);
            if (ancX > contX || ancY > contY || ancR < contR || ancB < contB)
            {
                return false;
            }
        }
    }
    return true;
}

void forgeCommit(MorphWindow& win)
{
    if (!win.hasRoot())
    {
        return;
    }

    ForgeWindowState& state = g_states[&win];
    if (state.m_forceGenSeen != g_forceGen)
    {
        state.m_firstFrame = true;
        state.m_forceGenSeen = g_forceGen;
    }

    win.renderer().ensureReady();

    auto& stats = win.dirtyStats();
    stats.reset();

    std::unordered_set<MorphNode*> paintBefore;
    walkTree(win.root(), [&](MorphNode* node) {
        if (node->isDirty(PaintDirty))
        {
            paintBefore.insert(node);
        }
    });

    win.root()->layoutIfNeeded(
        0.0f, 0.0f, win.contentWidth(), win.contentHeight(), &win.renderer(), &stats);
    stats.fullTreeCount = countNodes(win.root());

#ifdef MORPH_FEATURE_DEV
    syncPaintDirtyTree(win.root());
#endif

    int viewW = win.width();
    int viewH = win.height();
    int nodeCount = stats.fullTreeCount;
    DamageSet damage;
    std::vector<DamageRect> precise;
    int runningAnims = 0;

    walkTree(win.root(), [&](MorphNode* node) {
        for (auto& anim : node->m_animations)
        {
            if (anim.running && !anim.finished)
            {
                runningAnims++;
            }
        }
        if (hasColorCompositorAnim(node))
        {
            damageNodeScreen(damage, node);
            precise.push_back(nodeScreenBox(node));
        }
    });

    // NOTE: X/Y compositor movers deliberately do NOT force fullscreen.
    // Their old position is damaged via per-tick paint dirt (see
    // updateAnimations) and their new position is added present-side
    // (moverBox), where the interpolated offsets are actually known.

    bool nella = state.m_firstFrame || nodeCount != state.m_prevNodeCount ||
                 runningAnims > MAX_ANIMS_BEFORE_FULLSCREEN;
    state.m_shifts.clear();

    if (nella)
    {
        damage.setFullScreen();
        state.m_prevRects.clear();
        state.m_layers.clear();
        state.m_moverHave = false;
        state.m_moverIdx = -1;
    }
    else
    {
        std::vector<int> liveLayerIds;
        walkTree(win.root(), [&](MorphNode* node) {
            auto it = state.m_prevRects.find(node);
            bool boxChanged = it == state.m_prevRects.end();
            float oldScrollX = 0.0f;
            float oldScrollY = 0.0f;
            float oldContentW = 0.0f;
            float oldContentH = 0.0f;
            if (!boxChanged)
            {
                const PrevRect& prev = it->second;
                oldScrollX = prev.m_scrollX;
                oldScrollY = prev.m_scrollY;
                oldContentW = prev.m_contentW;
                oldContentH = prev.m_contentH;
                boxChanged = (static_cast<int>(node->x) != prev.m_box.x ||
                              static_cast<int>(node->y) != prev.m_box.y ||
                              static_cast<int>(node->w) != prev.m_box.w ||
                              static_cast<int>(node->h) != prev.m_box.h ||
                              static_cast<int>(node->scrollX) != static_cast<int>(prev.m_scrollX) ||
                              static_cast<int>(node->scrollY) != static_cast<int>(prev.m_scrollY) ||
                              static_cast<int>(node->contentW) != static_cast<int>(prev.m_contentW) ||
                              static_cast<int>(node->contentH) != static_cast<int>(prev.m_contentH));
            }
            if (boxChanged)
            {
                int scrX = static_cast<int>(node->x);
                int scrY = static_cast<int>(screenYOf(node));
                int scrW = static_cast<int>(node->w);
                int scrH = static_cast<int>(node->h);
                bool scrollOnly = false;
                if (it != state.m_prevRects.end() && node->scrollYEnabled &&
                    static_cast<int>(node->x) == it->second.m_box.x &&
                    static_cast<int>(node->y) == it->second.m_box.y &&
                    static_cast<int>(node->w) == it->second.m_box.w &&
                    static_cast<int>(node->h) == it->second.m_box.h &&
                    static_cast<int>(node->contentH) == static_cast<int>(oldContentH) &&
                    // The vertical shift fast path only handles pure-Y
                    // scrolls; any horizontal movement falls through to a
                    // full repaint below.
                    static_cast<int>(node->scrollX) == static_cast<int>(oldScrollX) &&
                    static_cast<int>(node->contentW) == static_cast<int>(oldContentW) &&
                    paintBefore.count(node) == 0 && shiftSafeAncestors(node))
                {
                    bool rounded = !node->style.borderRadius.isZero();
                    bool transformed = false;
#ifdef MORPH_FEATURE_TRANSFORM
                    transformed = node->style.transformSet;
#endif
                    ScrollShift shift;
                    if (detectScrollShift(
                            scrX,
                            scrY,
                            scrW,
                            scrH,
                            oldScrollY,
                            node->scrollY,
                            node->contentH,
                            transformed,
                            rounded,
                            shift))
                    {
                        float barW = 8.0f;
#ifdef MORPH_FEATURE_SCROLL
                        barW = node->style.scrollbarWidth;
#endif
                        // Shift the content area only: the scrollbar column
                        // is static (track) or fully damaged (thumbs), so
                        // moving it would smear translucent thumb pixels
                        // outside any damage rect.
                        shift.m_fullW = shift.m_w;
                        if (shift.m_w > static_cast<int>(barW) + 1)
                        {
                            shift.m_w -= static_cast<int>(barW);
                        }
                        state.m_shifts.push_back(shift);
                        DamageRect strip = exposedStrip(shift);
                        damage.add(strip);
                        precise.push_back(strip);
                        DamageRect oldThumb = thumbRect(
                            scrX,
                            scrY,
                            scrW,
                            scrH,
                            oldScrollY,
                            node->contentH,
                            barW);
                        DamageRect newThumb = thumbRect(
                            scrX,
                            scrY,
                            scrW,
                            scrH,
                            node->scrollY,
                            node->contentH,
                            barW);
                        damage.add(oldThumb);
                        damage.add(newThumb);
                        precise.push_back(oldThumb);
                        precise.push_back(newThumb);
                        node->clearDirty(PaintDirty);
                        node->clearDirty(ScrollDirty);
                        scrollOnly = true;
                    }
                }
                if (!scrollOnly)
                {
                    if (it != state.m_prevRects.end())
                    {
                        // Old pixels live at the old box under the CURRENT
                        // ancestor scrolls (shifts compose), so convert with
                        // today's offsets, not the stored ones.
                        float ancScroll = node->y - screenYOf(node);
                        DamageRect old = it->second.m_box;
                        old.y -= static_cast<int>(ancScroll);
                        damage.add(old);
                        precise.push_back(old);
                    }
                    damageNodeScreen(damage, node);
                    precise.push_back({scrX, scrY, scrW, scrH});
                    node->markDirty(PaintDirty);
                }
            }
            else if (paintBefore.count(node) != 0)
            {
                damageNodeScreen(damage, node);
                precise.push_back(nodeScreenBox(node));
            }
            else
            {
                node->clearDirty(PaintDirty);
                node->clearDirty(ScrollDirty);
            }
            bool transitioning = node->m_isTransitioning;
            bool colorAnim = hasColorCompositorAnim(node);
            bool positionAnim = hasPositionCompositorAnim(node);
            int layerId =
                static_cast<int>(reinterpret_cast<uintptr_t>(node) & 0x7fffffff);
            if (state.m_layers.shouldPromote(
                    layerId, static_cast<int>(node->w), static_cast<int>(node->h), transitioning,
                    colorAnim, positionAnim))
            {
                state.m_layers.update(
                    layerId, static_cast<int>(node->w), static_cast<int>(node->h),
                    state.m_frameId);
                liveLayerIds.push_back(layerId);
            }
        });
        // Preserved mover layer: the present-side fast path owns a
        // negative-keyed entry that commit's MorphNode walk never sees.
        // While any position anim still runs, keep it live so the cached
        // raster survives across commits; the anim end lets prune drop it.
        if (state.m_moverHave)
        {
            bool positionRunning = false;
            walkTree(win.root(), [&](MorphNode* node) {
                if (!positionRunning && hasPositionCompositorAnim(node))
                {
                    positionRunning = true;
                }
            });
            if (positionRunning)
            {
                liveLayerIds.push_back(state.m_moverKey);
                state.m_layers.refresh(state.m_moverKey, state.m_frameId);
            }
            else
            {
                state.m_moverHave = false;
                state.m_moverIdx = -1;
            }
        }
        state.m_layers.prune(liveLayerIds, state.m_frameId);
        // 2px safety margin past clip boundaries: commit truncates boxes
        // with (int) while render rounds them, which can disagree by ~1px
        // per edge (plus AA fringe); 2px provably covers the mismatch to a
        // sub-pixel sliver. Then clip to view.
        for (auto& rect : damage.rects)
        {
            rect.x -= 2;
            rect.y -= 2;
            rect.w += 4;
            rect.h += 4;
        }
        damage.clipTo(viewW, viewH);
    }

    if (static_cast<int>(state.m_prevRects.size()) > nodeCount * 2 + 32)
    {
        state.m_prevRects.clear();
    }

    state.m_firstFrame = false;
    state.m_prevNodeCount = nodeCount;
    walkTree(win.root(), [&](MorphNode* node) {
        PrevRect prev;
        prev.m_box = {static_cast<int>(node->x),
                      static_cast<int>(node->y),
                      static_cast<int>(node->w),
                      static_cast<int>(node->h)};
        prev.m_scrollX = node->scrollX;
        prev.m_scrollY = node->scrollY;
        prev.m_contentW = node->contentW;
        prev.m_contentH = node->contentH;
        state.m_prevRects[node] = prev;
    });

    stats.damageArea = damage.totalArea();
    state.m_damage = std::move(damage);
    state.m_precise = std::move(precise);

    recordPaintTree(win.root(), win.renderer(), stats);

    auto& channel = win.frameChannel();
    int backIdx = channel.backIndex.load();
    RenderFrame& frame = channel.backFrames[backIdx];
    frame.nodes.clear();
    frame.drawOps.clear();
    frame.animations.clear();
    frame.textOps.clear();
    frame.culledCount = 0;
    frame.frameId++;
    auto now = std::chrono::steady_clock::now().time_since_epoch();
    frame.timestamp = std::chrono::duration<double>(now).count();

    frame.viewW = win.contentWidth();
    frame.viewH = win.contentHeight();

    win.root()->flatten(frame, -1);
    stats.culledCount = frame.culledCount;

    channel.frontFrame.store(&frame, std::memory_order_release);
    channel.backIndex.store((backIdx + 1) % 2, std::memory_order_release);
    channel.framePending.store(true, std::memory_order_release);

    state.m_frameId++;
    win.clearPendingRender();
}

void forgePresent(MorphWindow& win, std::function<void(GLRenderer&, DirtyStats&)> overlayFn)
{
    if (!win.handle())
    {
        return;
    }

    auto& channel = win.frameChannel();
    while (!channel.frameInterpolated.load(std::memory_order_acquire))
    {
        std::this_thread::yield();
    }
    channel.frameInterpolated.store(false, std::memory_order_release);

    RenderFrame* frame = channel.frontFrame.load(std::memory_order_acquire);
    if (frame == nullptr)
    {
        return;
    }

    ForgeWindowState& state = g_states[&win];
    int w = win.width();
    int h = win.height();
    GLRenderer& renderer = win.renderer();

    float proj[16];
    {
        proj[0] = 2.0f / static_cast<float>(w);
        proj[4] = 0;
        proj[8] = 0;
        proj[12] = -1.0f;
        proj[1] = 0;
        proj[5] = -2.0f / static_cast<float>(h);
        proj[9] = 0;
        proj[13] = 1.0f;
        proj[2] = 0;
        proj[6] = 0;
        proj[10] = 1.0f;
        proj[14] = 0;
        proj[3] = 0;
        proj[7] = 0;
        proj[11] = 0;
        proj[15] = 1.0f;
    }

    bool fresh = ensureSurface(state, w, h);

    auto& stats = win.dirtyStats();
    bool fullscreen = state.m_damage.fullScreen || fresh;

    if (fresh)
    {
        state.m_firstFrame = true;
        fullscreen = true;
    }

    // Mover extension: compositor X/Y offsets are known only here
    // (post-interpolation), so each mover's new-position box joins the
    // damage now. Commit already damaged the old (base) boxes via
    // per-tick paint dirt; clear + raster below then cover both, and the
    // normal path draws the leaf at its offset position. Transformed
    // movers escalate to fullscreen (stale cull boxes) instead.
    if (!fullscreen)
    {
        for (const auto& node : frame->nodes)
        {
            float dx = node.animOffsetX;
            float dy = node.animOffsetY;
            if (dx == 0.0f && dy == 0.0f)
            {
                continue;
            }
            float screenY = node.y;
            bool transformed = false;
#ifdef MORPH_FEATURE_TRANSFORM
            transformed = node.transformSet;
#endif
            for (int p = node.parentId; p >= 0 && !transformed;)
            {
                if (p >= static_cast<int>(frame->nodes.size()))
                {
                    break;
                }
                const auto& parent = frame->nodes[static_cast<size_t>(p)];
                if (parent.scrollYEnabled && parent.contentH > parent.h)
                {
                    screenY -= parent.scrollY;
                }
#ifdef MORPH_FEATURE_TRANSFORM
                transformed = parent.transformSet;
#endif
                p = parent.parentId;
            }
            if (transformed)
            {
                fullscreen = true;
                break;
            }
            DamageRect base{
                static_cast<int>(node.x), static_cast<int>(screenY),
                static_cast<int>(node.w), static_cast<int>(node.h)};
            state.m_damage.add(moverBox(base, dx, dy));
        }
        if (fullscreen)
        {
            state.m_tiles.clear();
            state.m_shifts.clear();
            state.m_precise.clear();
        }
    }

    if (fullscreen)
    {
        state.m_tiles.clear();
        state.m_shifts.clear();
        state.m_precise.clear();
    }
    else
    {
        // Precise invalidation: only tiles overlapping boxes that actually
        // changed lose residency. Coarse merged rects (ancestor expansion,
        // bounding unions) still drive clear + raster below, but no longer
        // nuke tiles in merely-adjacent regions — stable tiles persist.
        for (const auto& exact : state.m_precise)
        {
            state.m_tiles.invalidateOverlapping(exact.x, exact.y, exact.w, exact.h);
        }
        for (const auto& dmg : state.m_damage.rects)
        {
            state.m_tiles.acquire({0, dmg.x, dmg.y, dmg.w, dmg.h});
        }
    }
    stats.tileCount = static_cast<int>(state.m_tiles.tileCount());
    stats.tileBytes = static_cast<int>(state.m_tiles.bytesUsed());

    // Mover-layer fast path (v1): a single eligible position-only leaf
    // reuses its cached raster (blended textured quad) instead of
    // re-rastering. The travel span joins the damage so every
    // intermediate position is repainted even when offsets jump; the
    // raster below restores the background excluding the leaf, then the
    // cached layer composites home with correct AA fringe blending.
    int moverIdx = -1;
    DamageRect moverNewBox{0, 0, 0, 0};
    GLuint moverTex = 0;
    bool useMoverLayer = false;
    if (!fullscreen)
    {
        MoverLayerGates gates;
        DamageRect baseBox{0, 0, 0, 0};
        DamageRect newBox{0, 0, 0, 0};
        int candidate = -1;
        if (tryMoverLayer(frame, candidate, gates, baseBox, newBox))
        {
            int key = -(candidate + 1);
            if (state.m_moverHave && state.m_moverIdx != candidate)
            {
                state.m_layers.drop(state.m_moverKey);
                state.m_moverHave = false;
                state.m_moverIdx = -1;
            }
            state.m_layers.update(key, gates.m_w + 2, gates.m_h + 2, state.m_frameId);
            int padW = gates.m_w + 2;
            int padH = gates.m_h + 2;
            GLuint fbo = state.m_layers.ensureSurface(key, padW, padH);
            RetainedLayer* layer = state.m_layers.find(key);
            const auto& fnode = frame->nodes[(size_t)candidate];
            const auto& op = frame->drawOps[(size_t)fnode.dlOffset];
            if (fbo != 0 && layer != nullptr)
            {
                bool fresh = !layer->m_captured || layer->m_capR != op.r ||
                             layer->m_capG != op.g || layer->m_capB != op.b ||
                             layer->m_capA != op.a;
                bool captured = !fresh;
                if (fresh)
                {
                    renderer.flush(proj);
                    if (win.captureNodeLayer(candidate, fbo, padW, padH))
                    {
                        layer->m_capR = op.r;
                        layer->m_capG = op.g;
                        layer->m_capB = op.b;
                        layer->m_capA = op.a;
                        layer->m_captured = true;
                        captured = true;
                    }
                    glBindFramebuffer(GL_FRAMEBUFFER, state.m_fbo);
                    glViewport(0, 0, w, h);
                    renderer.setFBHeight(h);
                    renderer.setProjection(proj);
                }
                if (captured)
                {
                    moverIdx = candidate;
                    moverNewBox = newBox;
                    moverTex = layer->m_texture;
                    useMoverLayer = true;
                    state.m_moverKey = key;
                    state.m_moverIdx = candidate;
                    state.m_moverHave = true;
                    state.m_damage.add(moverSpan(baseBox, fnode.animOffsetX,
                                                 fnode.animOffsetY));
                }
                else
                {
                    state.m_layers.drop(key);
                }
            }
            else
            {
                state.m_layers.drop(key);
            }
        }
    }
    stats.layerCount = static_cast<int>(state.m_layers.realCount());
    stats.layerBytes = static_cast<int>(state.m_layers.realBytes());

    if (!fullscreen && state.m_damage.empty() && state.m_shifts.empty())
    {
        if (!overlayFn)
        {
            stats.presentBytes = 0;
            return;
        }
        glBindFramebuffer(GL_READ_FRAMEBUFFER, state.m_fbo);
        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
        glReadBuffer(GL_COLOR_ATTACHMENT0);
        glDrawBuffer(GL_BACK);
        glBlitFramebuffer(0, 0, w, h, 0, 0, w, h, GL_COLOR_BUFFER_BIT, GL_NEAREST);
        glBindFramebuffer(GL_READ_FRAMEBUFFER, 0);
        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);

        overlayFn(renderer, stats);
        renderer.flush(proj);
        win.maybeScreenshot();
        glfwSwapBuffers(win.handle());
        return;
    }

    renderer.setFBHeight(h);
    glBindFramebuffer(GL_FRAMEBUFFER, state.m_fbo);
    glViewport(0, 0, w, h);
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);

    if (fullscreen)
    {
        float clear[4];
        win.bodyClearColor(clear);
        renderer.setClearColor(clear[0], clear[1], clear[2], clear[3]);
        renderer.clear();
        renderer.setProjection(proj);
        win.drawFrameNodes();
        stats.damageArea = w * h;
    }
    else
    {
        for (const auto& shift : state.m_shifts)
        {
            GLuint scratch = ensureShiftScratch(state, shift.m_w, shift.m_h);
            if (scratch != 0)
            {
                applyScrollShift(state.m_fbo, h, shift, scratch);
            }
            else
            {
                // No scratch surface: fall back to full-container damage
                // for this shift (always correct, just repaints more).
                int fullW = shift.m_fullW > 0 ? shift.m_fullW : shift.m_w;
                state.m_damage.add({shift.m_x, shift.m_y, fullW, shift.m_h});
            }
        }

        glColorMask(GL_FALSE, GL_FALSE, GL_FALSE, GL_FALSE);
        glClear(GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
        glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);

        float clear[4];
        win.bodyClearColor(clear);
        renderer.setClearColor(clear[0], clear[1], clear[2], clear[3]);
        glEnable(GL_SCISSOR_TEST);
        for (const auto& dmg : state.m_damage.rects)
        {
            glScissor(dmg.x, h - (dmg.y + dmg.h), dmg.w, dmg.h);
            glClear(GL_COLOR_BUFFER_BIT);
        }
        glDisable(GL_SCISSOR_TEST);

        renderer.setProjection(proj);
        if (useMoverLayer)
        {
            win.drawFrameNodesExcluding(moverIdx, &state.m_damage);
            renderer.drawTexture(moverTex, (float)moverNewBox.x - 1.0f,
                                 (float)moverNewBox.y - 1.0f,
                                 (float)moverNewBox.w + 2.0f,
                                 (float)moverNewBox.h + 2.0f);
            renderer.flush(proj);
        }
        else
        {
            win.drawFrameNodes(&state.m_damage);
        }

        stats.damageArea = state.m_damage.totalArea();
    }

    state.m_shifts.clear();

    glBindFramebuffer(GL_FRAMEBUFFER, 0);

    glBindFramebuffer(GL_READ_FRAMEBUFFER, state.m_fbo);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
    glReadBuffer(GL_COLOR_ATTACHMENT0);
    glDrawBuffer(GL_BACK);
    glBlitFramebuffer(0, 0, w, h, 0, 0, w, h, GL_COLOR_BUFFER_BIT, GL_NEAREST);
    glBindFramebuffer(GL_READ_FRAMEBUFFER, 0);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);

    stats.presentBytes = w * h * 4;

    if (overlayFn)
    {
        overlayFn(renderer, stats);
        renderer.flush(proj);
    }

    win.maybeScreenshot();
    glfwSwapBuffers(win.handle());
}

void drawDamageOverlay(MorphWindow& win, GLRenderer& r)
{
    auto it = g_states.find(&win);
    if (it == g_states.end())
    {
        return;
    }
    const DamageSet& damage = it->second.m_damage;
    float col[4] = {1.0f, 0.25f, 0.25f, 0.22f};
    if (damage.fullScreen)
    {
        r.drawRect(0.0f, 0.0f, (float)win.width(), (float)win.height(), col);
        return;
    }
    for (const auto& rect : damage.rects)
    {
        r.drawRect((float)rect.x, (float)rect.y, (float)rect.w, (float)rect.h, col);
    }
}

} // namespace forge
