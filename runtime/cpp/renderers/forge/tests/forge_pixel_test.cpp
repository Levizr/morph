// Headless pixel tests for forge raster paths (EGL + Mesa, no display).
// Drives the real MorphWindow::renderNode damage culling and
// forge::applyScrollShift against synthetic frames and diffs pixels:
// any culling/clear/shift mismatch shows up as a wrong pixel, which area
// benchmarks can never catch.
#include <EGL/egl.h>
#include <cstdio>
#include <cstring>
#include <limits>
#include <memory>
#include <string>
#include <vector>

#include "vendor/glad/glad.h"
#include "core/render_frame.h"
#include "core/window.h"
#include "forge/damage.h"
#include "forge/forge.h"
#include "forge/layer.h"
#include "forge/scroll_shift.h"

namespace
{

int g_checks = 0;
int g_failures = 0;

void check(bool ok, const char* name)
{
    g_checks++;
    if (!ok)
    {
        g_failures++;
        printf("[forge-pixel-test] FAIL %s\n", name);
    }
}

constexpr int WIN_W = 320;
constexpr int WIN_H = 200;

struct FBO
{
    GLuint m_fbo = 0;
    GLuint m_tex = 0;
    GLuint m_rbo = 0;
};

bool makeFBO(FBO& fbo)
{
    glGenFramebuffers(1, &fbo.m_fbo);
    glBindFramebuffer(GL_FRAMEBUFFER, fbo.m_fbo);
    glGenTextures(1, &fbo.m_tex);
    glBindTexture(GL_TEXTURE_2D, fbo.m_tex);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, WIN_W, WIN_H, 0, GL_RGBA,
                 GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D,
                           fbo.m_tex, 0);
    glGenRenderbuffers(1, &fbo.m_rbo);
    glBindRenderbuffer(GL_RENDERBUFFER, fbo.m_rbo);
    glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, WIN_W, WIN_H);
    glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT,
                              GL_RENDERBUFFER, fbo.m_rbo);
    bool ok = glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE;
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    return ok;
}

void blitFBO(const FBO& src, const FBO& dst)
{
    glBindFramebuffer(GL_READ_FRAMEBUFFER, src.m_fbo);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, dst.m_fbo);
    glReadBuffer(GL_COLOR_ATTACHMENT0);
    // Color + depth + stencil: the batcher depth-tests, so the copy must
    // carry the buffers a real retained surface would already hold.
    glBlitFramebuffer(0, 0, WIN_W, WIN_H, 0, 0, WIN_W, WIN_H,
                      GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT,
                      GL_NEAREST);
    glBindFramebuffer(GL_READ_FRAMEBUFFER, 0);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
}

std::vector<unsigned char> readFBO(const FBO& fbo)
{
    glBindFramebuffer(GL_FRAMEBUFFER, fbo.m_fbo);
    glPixelStorei(GL_PACK_ALIGNMENT, 1);
    std::vector<unsigned char> px((size_t)WIN_W * (size_t)WIN_H * 4);
    glReadPixels(0, 0, WIN_W, WIN_H, GL_RGBA, GL_UNSIGNED_BYTE, px.data());
    glBindFramebuffer(GL_FRAMEBUFFER, 0);
    return px;
}

// Clears one screen-space rect of the bound FBO to the body color,
// mirroring the forge present path (scissored color clear).
void clearRect(const DamageRect& r, const float clear[4])
{
    glEnable(GL_SCISSOR_TEST);
    glScissor(r.x, WIN_H - (r.y + r.h), r.w, r.h);
    glClearColor(clear[0], clear[1], clear[2], clear[3]);
    glClear(GL_COLOR_BUFFER_BIT);
    glDisable(GL_SCISSOR_TEST);
}

// Whole-surface depth+stencil reset (no color write), mirroring the
// forge present path before its scissored color clears.
void resetDepthStencil()
{
    glColorMask(GL_FALSE, GL_FALSE, GL_FALSE, GL_FALSE);
    glClear(GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
    glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);
}

int pixelDiff(const std::vector<unsigned char>& a, const std::vector<unsigned char>& b,
              int& outX, int& outY, int& outX1, int& outY1)
{
    // RGB allows +/-2 and alpha +/-16. Rationale: the SDF shader
    // antialiases rect edges (~1px fringe), so shifted-retained pixels
    // and freshly rastered ones legitimately differ by fringe noise at
    // container/row edges — same color family, never the tens-of-levels
    // gaps of real bugs (wrong rows, ghosts, missing content), which
    // still fail loudly. llvmpipe dithering is disabled in main() so
    // results are deterministic run to run.
    int diffs = 0;
    outX = -1;
    outY = -1;
    outX1 = -1;
    outY1 = -1;
    for (int y = 0; y < WIN_H; y++)
    {
        for (int x = 0; x < WIN_W; x++)
        {
            size_t i = ((size_t)y * (size_t)WIN_W + (size_t)x) * 4;
            int dr = (int)a[i] - (int)b[i];
            int dg = (int)a[i + 1] - (int)b[i + 1];
            int db = (int)a[i + 2] - (int)b[i + 2];
            int da = (int)a[i + 3] - (int)b[i + 3];
            if (dr > 2 || dr < -2 || dg > 2 || dg < -2 || db > 2 || db < -2 ||
                da > 16 || da < -16)
            {
                if (diffs == 0)
                {
                    outX = x;
                    outY = y;
                }
                outX1 = x;
                outY1 = y;
                diffs++;
            }
        }
    }
    return diffs;
}

void checkEqual(const std::vector<unsigned char>& a, const std::vector<unsigned char>& b,
                const char* name)
{
    int x = -1;
    int y = -1;
    int x1 = -1;
    int y1 = -1;
    int diffs = pixelDiff(a, b, x, y, x1, y1);
    if (diffs != 0)
    {
        // Buffer rows are bottom-up: report screen (top-down) coords.
        printf("[forge-pixel-test] %d wrong pixels screen x %d..%d y %d..%d\n", diffs, x,
               x1, WIN_H - 1 - y1, WIN_H - 1 - y);
        printf("[forge-pixel-test] all diffs (sx,sy,aref,atest):");
        int shown = 0;
        for (int yy = 0; yy < WIN_H && shown < 24; yy++)
        {
            for (int xx = 0; xx < WIN_W && shown < 24; xx++)
            {
                size_t j = ((size_t)yy * (size_t)WIN_W + (size_t)xx) * 4;
                if (a[j] != b[j] || a[j + 1] != b[j + 1] || a[j + 2] != b[j + 2] ||
                    a[j + 3] != b[j + 3])
                {
                    printf(" (%d,%d,%d,%d)", xx, WIN_H - 1 - yy, (int)a[j + 3],
                           (int)b[j + 3]);
                    shown++;
                }
            }
        }
        printf("\n");
        size_t i = ((size_t)y * (size_t)WIN_W + (size_t)x) * 4;
        printf("[forge-pixel-test] first diff at screen (%d,%d): ref=(%d,%d,%d,%d) test=(%d,%d,%d,%d)\n",
               x, WIN_H - 1 - y, a[i], a[i + 1], a[i + 2], a[i + 3], b[i], b[i + 1],
               b[i + 2], b[i + 3]);
        const char* tags[3] = {"test", "ref", "diff"};
        for (int k = 0; k < 3; k++)
        {
            char path[128];
            std::snprintf(path, sizeof(path), "/tmp/forge-pixel-fail-%s-%s.ppm", name,
                          tags[k]);
            printf("[forge-pixel-test] see %s\n", path);
            FILE* f = std::fopen(path, "wb");
            if (!f)
            {
                continue;
            }
            std::fprintf(f, "P6\n%d %d\n255\n", WIN_W, WIN_H);
            for (int row = WIN_H - 1; row >= 0; --row)
            {
                for (int col = 0; col < WIN_W; col++)
                {
                    size_t i = ((size_t)row * (size_t)WIN_W + (size_t)col) * 4;
                    unsigned char px[3] = {b[i], b[i + 1], b[i + 2]};
                    if (k == 1)
                    {
                        px[0] = a[i];
                        px[1] = a[i + 1];
                        px[2] = a[i + 2];
                    }
                    else if (k == 2)
                    {
                        bool same = a[i] == b[i] && a[i + 1] == b[i + 1] &&
                                    a[i + 2] == b[i + 2] && a[i + 3] == b[i + 3];
                        px[0] = same ? 0 : 255;
                        px[1] = 0;
                        px[2] = 0;
                    }
                    std::fwrite(px, 1, 3, f);
                }
            }
            std::fclose(f);
        }
    }
    check(diffs == 0, name);
}

// Layer-composite comparison: interior and background are pixel-gated
// like checkEqual (strict — a missing/misplaced/stale layer fails via
// its interior), while the mover's 1px border ring allows +/-32. The
// ring is SDF corner/edge fringe whose float phase legitimately differs
// between a padded-FBO capture and a fresh interior raster (measured
// <=29 RGB on exact corners); real composite bugs (background instead
// of leaf, wrong box, flipped content) differ by 100+ inside and still
// fail loudly. Rest frames never take the layer path (offset 0), so
// static parity stays exact-gated.
void checkEqualLayer(const std::vector<unsigned char>& a, const std::vector<unsigned char>& b,
                     const DamageRect& box, const char* name)
{
    int diffs = 0;
    int wide = 0;
    for (int y = 0; y < WIN_H; y++)
    {
        for (int x = 0; x < WIN_W; x++)
        {
            int sx = x;
            int sy = WIN_H - 1 - y;
            bool ring = sx >= box.x && sx < box.x + box.w && sy >= box.y &&
                        sy < box.y + box.h &&
                        (sx == box.x || sx == box.x + box.w - 1 || sy == box.y ||
                         sy == box.y + box.h - 1);
            size_t i = ((size_t)y * (size_t)WIN_W + (size_t)x) * 4;
            int dr = (int)a[i] - (int)b[i];
            int dg = (int)a[i + 1] - (int)b[i + 1];
            int db = (int)a[i + 2] - (int)b[i + 2];
            int da = (int)a[i + 3] - (int)b[i + 3];
            int tol = ring ? 32 : 2;
            int tolA = ring ? 32 : 16;
            if (dr > tol || dr < -tol || dg > tol || dg < -tol || db > tol ||
                db < -tol || da > tolA || da < -tolA)
            {
                diffs++;
                if (ring)
                {
                    wide++;
                }
            }
        }
    }
    if (diffs != 0)
    {
        printf("[forge-pixel-test] %d layer-diffs (%d ring) box=(%d,%d,%d,%d)\n", diffs,
               wide, box.x, box.y, box.w, box.h);
    }
    check(diffs == 0, name);
}

void paintRect(RenderFrame& frame, int nodeIdx, float x, float y, float w, float h,
               const float color[4])
{
    FlatRenderNode& node = frame.nodes[(size_t)nodeIdx];
    node.dlOffset = (int)frame.drawOps.size();
    DrawOp op;
    float c[4] = {color[0], color[1], color[2], color[3]};
    op.setRect(x, y, w, h, c);
    frame.drawOps.push_back(op);
    node.dlCount = (int)frame.drawOps.size() - node.dlOffset;
}

int addNode(RenderFrame& frame, int parentId, float x, float y, float w, float h)
{
    FlatRenderNode node;
    node.id = (int)frame.nodes.size();
    node.parentId = parentId;
    node.x = x;
    node.y = y;
    node.w = w;
    node.h = h;
    node.bgColor[0] = 0.0f;
    node.bgColor[1] = 0.0f;
    node.bgColor[2] = 0.0f;
    node.bgColor[3] = 0.0f;
    node.borderRadius = 0.0f;
    node.borderWidth = 0.0f;
    node.borderColor[0] = 0.0f;
    node.borderColor[1] = 0.0f;
    node.borderColor[2] = 0.0f;
    node.borderColor[3] = 0.0f;
    node.color[0] = 0.0f;
    node.color[1] = 0.0f;
    node.color[2] = 0.0f;
    node.color[3] = 1.0f;
    node.boxSizing = CSS::BoxSizing::ContentBox;
    node.display = CSS::Display::Block;
    node.position = CSS::Position::Static;
    node.overflow = CSS::Overflow::Visible;
    node.fontSize = 16.0f;
    node.textAlign = CSS::TextAlign::Left;
    node.fontWeight = CSS::FontWeight::Normal;
    node.scrollY = 0.0f;
    node.contentH = 0.0f;
    node.scrollEnabled = false;
    node.scrollbarWidth = 8.0f;
    node.scrollbarTrackColor[0] = 0.85f;
    node.scrollbarTrackColor[1] = 0.85f;
    node.scrollbarTrackColor[2] = 0.85f;
    node.scrollbarTrackColor[3] = 0.4f;
    node.scrollbarThumbColor[0] = 0.5f;
    node.scrollbarThumbColor[1] = 0.5f;
    node.scrollbarThumbColor[2] = 0.5f;
    node.scrollbarThumbColor[3] = 0.6f;
    node.scrollbarBorderRadius = 4.0f;
    node.isTransitioning = false;
    node.hasLayoutTransition = false;
    node.opacity = 1.0f;
    node.animOffsetX = 0.0f;
    node.animOffsetY = 0.0f;
    node.animOpacity = 1.0f;
    node.textOpOffset = 0;
    node.textOpCount = 0;
    node.dlOffset = 0;
    node.dlCount = 0;
    node.clipText = false;
    node.textClipPadX = 0.0f;
    node.selX0 = std::numeric_limits<float>::quiet_NaN();
    node.selX1 = std::numeric_limits<float>::quiet_NaN();
    node.caretX = std::numeric_limits<float>::quiet_NaN();
    node.caretY = 0.0f;
    node.caretH = 0.0f;
    int idx = (int)frame.nodes.size();
    frame.nodes.push_back(node);
    if (parentId >= 0)
    {
        frame.nodes[(size_t)parentId].children.push_back(idx);
    }
    return idx;
}

// Scene: dark body, one scroll container (300x180 at 10,10, content 540
// tall in 45px rows), optional scroll offset. Mirrors examples/forge.
void buildScene(RenderFrame& frame, float scrollY)
{
    frame.nodes.clear();
    frame.drawOps.clear();
    frame.animations.clear();
    frame.textOps.clear();
    frame.viewW = (float)WIN_W;
    frame.viewH = (float)WIN_H;
    frame.culledCount = 0;

    const float body[4] = {0.063f, 0.078f, 0.094f, 1.0f};
    int root = addNode(frame, -1, 0.0f, 0.0f, (float)WIN_W, (float)WIN_H);
    frame.nodes[(size_t)root].bgColor[0] = body[0];
    frame.nodes[(size_t)root].bgColor[1] = body[1];
    frame.nodes[(size_t)root].bgColor[2] = body[2];
    frame.nodes[(size_t)root].bgColor[3] = body[3];
    paintRect(frame, root, 0.0f, 0.0f, (float)WIN_W, (float)WIN_H, body);

    const float listBg[4] = {0.086f, 0.106f, 0.133f, 1.0f};
    int list = addNode(frame, root, 10.0f, 10.0f, 300.0f, 180.0f);
    if (scrollY < 0.0f)
    {
        // Static control: no clipping, no scrolling.
        frame.nodes[(size_t)list].overflow = CSS::Overflow::Visible;
    }
    else
    {
        frame.nodes[(size_t)list].overflow = CSS::Overflow::Hidden;
        frame.nodes[(size_t)list].scrollEnabled = true;
        frame.nodes[(size_t)list].scrollY = scrollY;
    }
    frame.nodes[(size_t)list].contentH = 540.0f;
    frame.nodes[(size_t)list].scrollbarWidth = 8.0f;
    paintRect(frame, list, 10.0f, 10.0f, 300.0f, 180.0f, listBg);

    const float rowA[4] = {0.106f, 0.129f, 0.161f, 1.0f};
    const float rowB[4] = {0.129f, 0.153f, 0.188f, 1.0f};
    for (int i = 0; i < 12; i++)
    {
        float ry = (float)(i * 45);
        int row = addNode(frame, list, 20.0f, ry, 280.0f, 45.0f);
        const float* c = (i % 2 == 0) ? rowA : rowB;
        paintRect(frame, row, 20.0f, ry, 280.0f, 45.0f, c);
    }
}

DamageRect thumbFor(float scrollY)
{
    float thumbH = (180.0f / 540.0f) * 180.0f;
    float thumbY = 10.0f + (scrollY / (540.0f - 180.0f)) * (180.0f - thumbH);
    return {300, (int)thumbY, 9, (int)(thumbH + 1.0f)};
}

void margin2(DamageSet& damage)
{
    for (auto& r : damage.rects)
    {
        r.x -= 2;
        r.y -= 2;
        r.w += 4;
        r.h += 4;
    }
    damage.clipTo(WIN_W, WIN_H);
}

// Main-window ortho projection (mirrors MorphWindow::ortho for
// 0,WIN_W,WIN_H,0,-1,1): flushes textured-quad composites with the same
// matrix production presents with.
void mainProj(float proj[16])
{
    proj[0] = 2.0f / (float)WIN_W;
    proj[1] = 0;
    proj[2] = 0;
    proj[3] = 0;
    proj[4] = 0;
    proj[5] = -2.0f / (float)WIN_H;
    proj[6] = 0;
    proj[7] = 0;
    proj[8] = 0;
    proj[9] = 0;
    proj[10] = -1.0f;
    proj[11] = 0;
    proj[12] = -1.0f;
    proj[13] = 1.0f;
    proj[14] = 0;
    proj[15] = 1.0f;
}

// Mover scene: body root + one opaque leaf at (20,20,100,40) with a
// compositor X/Y offset. The leaf is a single Rect (capturable), no
// text, no overlap with anything but the root background.
int buildMoverScene(RenderFrame& frame, float dx, float dy, bool colorAnim = false)
{
    frame.nodes.clear();
    frame.drawOps.clear();
    frame.animations.clear();
    frame.textOps.clear();
    frame.viewW = (float)WIN_W;
    frame.viewH = (float)WIN_H;
    frame.culledCount = 0;

    const float body[4] = {0.063f, 0.078f, 0.094f, 1.0f};
    int root = addNode(frame, -1, 0.0f, 0.0f, (float)WIN_W, (float)WIN_H);
    frame.nodes[(size_t)root].bgColor[0] = body[0];
    frame.nodes[(size_t)root].bgColor[1] = body[1];
    frame.nodes[(size_t)root].bgColor[2] = body[2];
    frame.nodes[(size_t)root].bgColor[3] = body[3];
    paintRect(frame, root, 0.0f, 0.0f, (float)WIN_W, (float)WIN_H, body);

    const float moverColor[4] = {0.80f, 0.20f, 0.20f, 1.0f};
    int mover = addNode(frame, root, 20.0f, 20.0f, 100.0f, 40.0f);
    paintRect(frame, mover, 20.0f, 20.0f, 100.0f, 40.0f, moverColor);
    frame.nodes[(size_t)mover].animOffsetX = dx;
    frame.nodes[(size_t)mover].animOffsetY = dy;

    AnimationState anim;
    anim.nodeId = mover;
    anim.prop = CompositorAnimProperty::X;
    anim.from = 20.0f;
    anim.to = 20.0f + dx;
    anim.startTime = 0.0;
    anim.duration = 1.0f;
    anim.easing = 0;
    anim.running = true;
    frame.animations.push_back(anim);
    if (colorAnim)
    {
        AnimationState color;
        color.nodeId = mover;
        color.prop = CompositorAnimProperty::BgColorR;
        color.from = 0.8f;
        color.to = 0.2f;
        color.startTime = 0.0;
        color.duration = 1.0f;
        color.easing = 0;
        color.running = true;
        frame.animations.push_back(color);
    }
    return mover;
}

} // namespace

int main()
{
    EGLDisplay dpy = eglGetDisplay(EGL_DEFAULT_DISPLAY);
    if (dpy == EGL_NO_DISPLAY)
    {
        printf("[forge-pixel-test] SKIP: no EGL display\n");
        return 0;
    }
    if (!eglInitialize(dpy, nullptr, nullptr))
    {
        printf("[forge-pixel-test] SKIP: EGL init failed\n");
        return 0;
    }
    const EGLint cfg[] = {EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE,
                          EGL_OPENGL_BIT, EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8,
                          EGL_BLUE_SIZE, 8, EGL_NONE};
    EGLConfig config = nullptr;
    EGLint n = 0;
    if (!eglChooseConfig(dpy, cfg, &config, 1, &n) || n < 1)
    {
        printf("[forge-pixel-test] SKIP: no EGL config\n");
        return 0;
    }
    const EGLint pb[] = {EGL_WIDTH, 16, EGL_HEIGHT, 16, EGL_NONE};
    EGLSurface surf = eglCreatePbufferSurface(dpy, config, pb);
    if (surf == EGL_NO_SURFACE || !eglBindAPI(EGL_OPENGL_API))
    {
        printf("[forge-pixel-test] SKIP: no EGL surface\n");
        return 0;
    }
    EGLContext ctx = eglCreateContext(dpy, config, EGL_NO_CONTEXT, nullptr);
    if (ctx == EGL_NO_CONTEXT || !eglMakeCurrent(dpy, surf, surf, ctx))
    {
        printf("[forge-pixel-test] SKIP: no EGL context\n");
        return 0;
    }
    gladLoadGLLoader((GLADloadproc)eglGetProcAddress);

    // Production always blends (GLRenderer::clear and the forge present
    // path both enable it); without this the test draws in a state no
    // shipped frame ever sees (notably wrong alpha on fringes/tracks).
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    // Deterministic pixels: llvmpipe dithering perturbs LSBs run to run,
    // which breaks exact-color assertions (row counts). Prod keeps dither
    // for visual quality; the test needs repeatability instead.
    glDisable(GL_DITHER);

    auto win = std::make_unique<MorphWindow>("px", WIN_W, WIN_H, false);
    win->renderer().ensureReady();

    FBO ref;
    FBO work;
    check(makeFBO(ref) && makeFBO(work), "fbo-complete");

    const float body[4] = {0.063f, 0.078f, 0.094f, 1.0f};

    // T1: full draw is the reference everything else must reproduce.
    RenderFrame frame120;
    buildScene(frame120, 120.0f);
    win->frameChannel().frontFrame.store(&frame120, std::memory_order_release);
    glBindFramebuffer(GL_FRAMEBUFFER, ref.m_fbo);
    glViewport(0, 0, WIN_W, WIN_H);
    glClearColor(body[0], body[1], body[2], body[3]);
    // Full clear like production (color + depth + stencil): the batcher
    // depth-tests, so garbage depth would cull nondeterministically.
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
    win->drawFrameNodes();
    std::vector<unsigned char> refPx = readFBO(ref);
    {
        // Ground truth for debugging (overwritten every run).
        FILE* f = std::fopen("/tmp/forge-pixel-ref.ppm", "wb");
        if (f)
        {
            std::fprintf(f, "P6\n%d %d\n255\n", WIN_W, WIN_H);
            for (int row = WIN_H - 1; row >= 0; --row)
            {
                for (int col = 0; col < WIN_W; col++)
                {
                    size_t i = ((size_t)row * (size_t)WIN_W + (size_t)col) * 4;
                    std::fwrite(&refPx[i], 1, 3, f);
                }
            }
            std::fclose(f);
        }
    }

    // T0: full draw is deterministic across FBOs (same frame, twice).
    {
        FBO det;
        check(makeFBO(det), "fbo-det-complete");
        glBindFramebuffer(GL_FRAMEBUFFER, det.m_fbo);
        glViewport(0, 0, WIN_W, WIN_H);
        glClearColor(body[0], body[1], body[2], body[3]);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
        win->drawFrameNodes();
        checkEqual(readFBO(det), refPx, "full-draw-deterministic");

        // Full-window damage agrees with no damage clip.
        blitFBO(ref, work);
        DamageSet full;
        full.add({0, 0, WIN_W, WIN_H});
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        resetDepthStencil();
        for (const auto& r : full.rects)
        {
            clearRect(r, body);
        }
        win->drawFrameNodes(&full);
        checkEqual(readFBO(work), refPx, "full-damage-agrees");

        // A bare blit reproduces every pixel (isolates blitFBO itself).
        blitFBO(ref, work);
        checkEqual(readFBO(work), refPx, "blit-identical");
    }

    // T1b: stage bisection for damage draws — same damage, but skip the
    // pre-clear (retained pixels stay; only damage-touching nodes draw).
    {
        blitFBO(ref, work);
        DamageSet damage;
        damage.add({40, 40, 120, 80});
        margin2(damage);
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        resetDepthStencil();
        win->drawFrameNodes(&damage);
        checkEqual(readFBO(work), refPx, "damage-noclear-identical");
    }

    // T2: damage-limited re-raster reproduces the reference exactly.
    {
        blitFBO(ref, work);
        DamageSet damage;
        damage.add({40, 40, 120, 80});
        margin2(damage);
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        resetDepthStencil();
        for (const auto& r : damage.rects)
        {
            clearRect(r, body);
        }
        win->drawFrameNodes(&damage);
        {
            glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
            glPixelStorei(GL_PACK_ALIGNMENT, 1);
            std::vector<unsigned char> st((size_t)WIN_W * (size_t)WIN_H);
            glReadPixels(0, 0, WIN_W, WIN_H, GL_STENCIL_INDEX, GL_UNSIGNED_BYTE,
                         st.data());
            int marked = 0;
            for (auto v : st)
            {
                if (v == 0x80)
                {
                    marked++;
                }
            }
            printf("[forge-pixel-test] stencil-marked=%d (damage ~10416)\n", marked);
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
        }
        checkEqual(readFBO(work), refPx, "damage-raster-identical");
    }

    // T3: scroll-shift end to end — shift retained pixels, repaint only
    // the exposed strip (+ thumbs, like production), match a full draw
    // at the new offset.
    {
        RenderFrame frame200;
        buildScene(frame200, 200.0f);
        forge::ScrollShift shift;
        bool ok = forge::detectScrollShift(10, 10, 300, 180, 120.0f, 200.0f, 540.0f,
                                           false, false, shift);
        check(ok, "shift-detected");
        blitFBO(ref, work);
        FBO scratch;
        check(makeFBO(scratch), "scratch-complete");
        // Production narrows the shift to the content area (scrollbar
        // column excluded); mirror it so the test exercises the real path.
        shift.m_fullW = shift.m_w;
        shift.m_w -= 8;
        forge::applyScrollShift(work.m_fbo, WIN_H, shift, scratch.m_fbo);
        win->frameChannel().frontFrame.store(&frame200, std::memory_order_release);
        DamageSet damage;
        damage.add(forge::exposedStrip(shift));
        damage.add(thumbFor(120.0f));
        damage.add(thumbFor(200.0f));
        margin2(damage);
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        resetDepthStencil();
        for (const auto& r : damage.rects)
        {
            clearRect(r, body);
        }
        win->drawFrameNodes(&damage);
        std::vector<unsigned char> shifted = readFBO(work);

        FBO ref200;
        check(makeFBO(ref200), "fbo200-complete");
        glBindFramebuffer(GL_FRAMEBUFFER, ref200.m_fbo);
        glViewport(0, 0, WIN_W, WIN_H);
        glClearColor(body[0], body[1], body[2], body[3]);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
        glBindFramebuffer(GL_FRAMEBUFFER, ref200.m_fbo);
        win->drawFrameNodes();
        checkEqual(shifted, readFBO(ref200), "scroll-shift-identical");
        // Same full draw but through the damage path (full-window mask):
        // must also match, proving the mask itself is pixel-neutral.
        {
            FBO refMasked;
            check(makeFBO(refMasked), "fbomasked-complete");
            glBindFramebuffer(GL_FRAMEBUFFER, refMasked.m_fbo);
            glViewport(0, 0, WIN_W, WIN_H);
            glClearColor(body[0], body[1], body[2], body[3]);
            glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
            DamageSet fullDmg;
            fullDmg.add({0, 0, WIN_W, WIN_H});
            win->drawFrameNodes(&fullDmg);
            checkEqual(readFBO(refMasked), readFBO(ref200), "masked-full-identical");
        }
    }

    // T4: exclusion skips the leaf. Control first: normal draw repaints
    // the cleared region with the leaf's color. Then excluding the leaf
    // leaves the container background instead — everything else identical.
    // Row 3 (index 5) covers screen x 20..300, y 15..60; the damage rect
    // (with margin) is x 18..122, y 18..62, so assert on x 20..122,
    // y 18..60 where only row 3 can paint.
    {
        win->frameChannel().frontFrame.store(&frame120, std::memory_order_release);
        check(frame120.nodes.size() > 5 && frame120.nodes[5].y == 135.0f, "leaf-box");
        const int leaf = 5;
        auto at = [&](const std::vector<unsigned char>& px, int x, int y, unsigned char out[3]) {
            size_t i = ((size_t)(WIN_H - 1 - y) * (size_t)WIN_W + (size_t)x) * 4;
            out[0] = px[i];
            out[1] = px[i + 1];
            out[2] = px[i + 2];
        };
        unsigned char rowColor[3];
        unsigned char bgColor[3];
        at(refPx, 50, 30, rowColor);
        at(refPx, 15, 100, bgColor);
        auto nearColor = [&](unsigned char p[3], unsigned char c[3]) {
            // Same +/-2 fringe tolerance as pixelDiff: SDF edges wobble
            // LSBs; row/bg palettes stay 5+ apart per channel.
            for (int k = 0; k < 3; k++)
            {
                int d = (int)p[k] - (int)c[k];
                if (d > 2 || d < -2)
                {
                    return false;
                }
            }
            return true;
        };
        auto countColor = [&](const std::vector<unsigned char>& px, unsigned char c[3]) {
            int n = 0;
            for (int y = 18; y < 60; y++)
            {
                for (int x = 21; x < 122; x++)
                {
                    unsigned char p[3];
                    at(px, x, y, p);
                    if (nearColor(p, c))
                    {
                        n++;
                    }
                }
            }
            return n;
        };
        const int total = (122 - 21) * (60 - 18);

        blitFBO(ref, work);
        DamageSet damage;
        damage.add({20, 20, 100, 40});
        margin2(damage);
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        resetDepthStencil();
        for (const auto& r : damage.rects)
        {
            clearRect(r, body);
        }
        win->drawFrameNodes(&damage);
        check(countColor(readFBO(work), rowColor) == total, "control-repaints-leaf");

        blitFBO(ref, work);
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        resetDepthStencil();
        for (const auto& r : damage.rects)
        {
            clearRect(r, body);
        }
        win->drawFrameNodesExcluding(leaf, &damage);
        std::vector<unsigned char> excl = readFBO(work);
        check(countColor(excl, rowColor) == 0, "exclusion-skips-leaf");
        check(countColor(excl, bgColor) == total, "exclusion-keeps-background");
    }

    // T5: blit mapping is exact — unique stripe colors prove every
    // destination row comes from the right source row.
    {
        FBO src;
        FBO dst;
        check(makeFBO(src) && makeFBO(dst), "fbo-shift-complete");
        glBindFramebuffer(GL_FRAMEBUFFER, src.m_fbo);
        glViewport(0, 0, WIN_W, WIN_H);
        glDisable(GL_SCISSOR_TEST);
        for (int y = 0; y < WIN_H; y += 10)
        {
            float c[4] = {(float)(y % 256) / 255.0f, 0.0f, 0.0f, 1.0f};
            glScissor(0, y, WIN_W, 10);
            glEnable(GL_SCISSOR_TEST);
            glClearColor(c[0], c[1], c[2], c[3]);
            glClear(GL_COLOR_BUFFER_BIT);
        }
        glDisable(GL_SCISSOR_TEST);
        forge::ScrollShift shift;
        bool ok = forge::detectScrollShift(0, 0, WIN_W, WIN_H, 0.0f, 80.0f, 4000.0f,
                                           false, false, shift);
        check(ok, "shift-80-detected");
        blitFBO(src, dst);
        FBO scratch;
        check(makeFBO(scratch), "shift-scratch-complete");
        forge::applyScrollShift(dst.m_fbo, WIN_H, shift, scratch.m_fbo);
        std::vector<unsigned char> srcPx = readFBO(src);
        std::vector<unsigned char> dstPx = readFBO(dst);
        // Content moved up 80: dst screen rows 10..109 must equal src
        // rows 90..189; rows below are untouched src copies (same check
        // pattern, covered implicitly by the band edges).
        int bad = 0;
        for (int r = 10; r < 110 && bad == 0; r++)
        {
            for (int x = 0; x < WIN_W; x++)
            {
                size_t di = ((size_t)(WIN_H - 1 - r) * (size_t)WIN_W + (size_t)x) * 4;
                size_t si =
                    ((size_t)(WIN_H - 1 - (r + 80)) * (size_t)WIN_W + (size_t)x) * 4;
                if (dstPx[di] != srcPx[si])
                {
                    bad++;
                    break;
                }
            }
        }
        check(bad == 0, "shift-maps-rows");
    }

    // T6: mover-layer end to end — capture once, background-restore +
    // blit at two positions, both pixel-identical to full draws.
    // Ineligible scenes (two movers, color anim, sibling overlap)
    // must decline the fast path.
    {
        RenderFrame frameB;
        int moverB = buildMoverScene(frameB, 60.0f, 0.0f);
        win->frameChannel().frontFrame.store(&frameB, std::memory_order_release);
        FBO refB;
        check(makeFBO(refB), "fbo-moverB-complete");
        glBindFramebuffer(GL_FRAMEBUFFER, refB.m_fbo);
        glViewport(0, 0, WIN_W, WIN_H);
        glClearColor(body[0], body[1], body[2], body[3]);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
        win->drawFrameNodes();
        std::vector<unsigned char> refBPx = readFBO(refB);

        int outIdx = -1;
        forge::MoverLayerGates gates;
        DamageRect baseBox{0, 0, 0, 0};
        DamageRect newBox{0, 0, 0, 0};
        check(forge::tryMoverLayer(&frameB, outIdx, gates, baseBox, newBox),
              "mover-layer-eligible");
        check(outIdx == moverB, "mover-layer-index");
        check(baseBox.x == 20 && baseBox.y == 20 && baseBox.w == 100 && baseBox.h == 40,
              "mover-layer-base");
        check(newBox.x == 80 && newBox.y == 20, "mover-layer-new");

        RenderFrame frameA;
        buildMoverScene(frameA, 0.0f, 0.0f);
        win->frameChannel().frontFrame.store(&frameA, std::memory_order_release);
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        glViewport(0, 0, WIN_W, WIN_H);
        glClearColor(body[0], body[1], body[2], body[3]);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
        win->drawFrameNodes();

        forge::LayerPool pool;
        int key = -(outIdx + 1);
        pool.update(key, gates.m_w + 2, gates.m_h + 2, 1);
        GLuint layerFbo = pool.ensureSurface(key, gates.m_w + 2, gates.m_h + 2);
        check(layerFbo != 0, "mover-layer-surface");
        win->frameChannel().frontFrame.store(&frameB, std::memory_order_release);
        check(win->captureNodeLayer(outIdx, layerFbo, gates.m_w + 2, gates.m_h + 2),
              "mover-layer-captured");

        DamageSet damage;
        damage.add(moverSpan(baseBox, 60.0f, 0.0f));
        margin2(damage);
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        resetDepthStencil();
        for (const auto& r : damage.rects)
        {
            clearRect(r, body);
        }
        win->drawFrameNodesExcluding(outIdx, &damage);
        win->renderer().drawTexture(pool.find(key)->m_texture, (float)newBox.x - 1.0f,
                                    (float)newBox.y - 1.0f, (float)newBox.w + 2.0f,
                                    (float)newBox.h + 2.0f);
        {
            float proj[16];
            mainProj(proj);
            win->renderer().setProjection(proj);
            win->renderer().flush(proj);
        }
        checkEqualLayer(readFBO(work), refBPx, newBox, "mover-layer-identical");

        RenderFrame frameC;
        buildMoverScene(frameC, 120.0f, 0.0f);
        win->frameChannel().frontFrame.store(&frameC, std::memory_order_release);
        FBO refC;
        check(makeFBO(refC), "fbo-moverC-complete");
        glBindFramebuffer(GL_FRAMEBUFFER, refC.m_fbo);
        glViewport(0, 0, WIN_W, WIN_H);
        glClearColor(body[0], body[1], body[2], body[3]);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
        win->drawFrameNodes();
        std::vector<unsigned char> refCPx = readFBO(refC);

        int outIdxC = -1;
        forge::MoverLayerGates gatesC;
        DamageRect baseC{0, 0, 0, 0};
        DamageRect newC{0, 0, 0, 0};
        check(forge::tryMoverLayer(&frameC, outIdxC, gatesC, baseC, newC),
              "mover-layer-eligible-again");
        check(outIdxC == outIdx && gatesC.m_w == gates.m_w, "mover-layer-stable");
        DamageSet damageC;
        damageC.add(moverSpan(baseC, 120.0f, 0.0f));
        margin2(damageC);
        glBindFramebuffer(GL_FRAMEBUFFER, work.m_fbo);
        resetDepthStencil();
        for (const auto& r : damageC.rects)
        {
            clearRect(r, body);
        }
        win->drawFrameNodesExcluding(outIdxC, &damageC);
        win->renderer().drawTexture(pool.find(key)->m_texture, (float)newC.x - 1.0f,
                                    (float)newC.y - 1.0f, (float)newC.w + 2.0f,
                                    (float)newC.h + 2.0f);
        {
            float proj[16];
            mainProj(proj);
            win->renderer().setProjection(proj);
            win->renderer().flush(proj);
        }
        checkEqualLayer(readFBO(work), refCPx, newC, "mover-layer-reused");

        RenderFrame frameTwo;
        buildMoverScene(frameTwo, 60.0f, 0.0f);
        int extra = addNode(frameTwo, 0, 200.0f, 100.0f, 60.0f, 30.0f);
        const float extraColor[4] = {0.2f, 0.6f, 0.2f, 1.0f};
        paintRect(frameTwo, extra, 200.0f, 100.0f, 60.0f, 30.0f, extraColor);
        frameTwo.nodes[(size_t)extra].animOffsetX = 10.0f;
        int noIdx = -1;
        forge::MoverLayerGates noGates;
        DamageRect noBase{0, 0, 0, 0};
        DamageRect noNew{0, 0, 0, 0};
        check(!forge::tryMoverLayer(&frameTwo, noIdx, noGates, noBase, noNew),
              "mover-layer-declines-multi");

        RenderFrame frameColor;
        buildMoverScene(frameColor, 60.0f, 0.0f, true);
        check(!forge::tryMoverLayer(&frameColor, noIdx, noGates, noBase, noNew),
              "mover-layer-declines-color");

        RenderFrame frameOverlap;
        int moverO = buildMoverScene(frameOverlap, 60.0f, 0.0f);
        (void)moverO;
        int sib = addNode(frameOverlap, 0, 90.0f, 20.0f, 60.0f, 40.0f);
        const float sibColor[4] = {0.2f, 0.2f, 0.8f, 1.0f};
        paintRect(frameOverlap, sib, 90.0f, 20.0f, 60.0f, 40.0f, sibColor);
        check(!forge::tryMoverLayer(&frameOverlap, noIdx, noGates, noBase, noNew),
              "mover-layer-declines-overlap");
    }

    printf("[forge-pixel-test] %d checks, %d failures\n", g_checks, g_failures);
    return g_failures == 0 ? 0 : 1;
}
