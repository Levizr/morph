#pragma once

#ifdef MORPH_FEATURE_SHADOW
// Shared box-shadow paint helpers: outset pass before the box, inset pass
// after (clipped to the padding box so borders stay clean). Both the
// immediate path (border_paint.h, unresolved style + UnitEnv) and the op
// path (window.cpp, baked FlatRenderNode px) funnel through these with
// already-resolved shadows.

#include "../core/renderer.h"
#include "../style/style.h"

namespace shadow_paint
{

// Split a resolved list into outset/inset subsets (CSS paints outset
// under the box, inset over the background — separate draws).
inline void splitShadows(const BoxShadow* in, int n, BoxShadow* outSet,
                         int& nOut, BoxShadow* inSet, int& nIn)
{
    nOut = nIn = 0;
    for (int i = 0; i < n; i++)
    {
        if (in[i].inset)
        {
            if (nIn < MORPH_SHADOW_MAX)
            {
                inSet[nIn++] = in[i];
            }
        }
        else
        {
            if (nOut < MORPH_SHADOW_MAX)
            {
                outSet[nOut++] = in[i];
            }
        }
    }
}

// Outset pass over the border box (bx,by,bw,bh) with outer radii.
inline void paintOutset(Renderer& r, float bx, float by, float bw, float bh,
                        const float radii[4], const BoxShadow* shadows, int n)
{
    BoxShadow out[MORPH_SHADOW_MAX];
    BoxShadow in[MORPH_SHADOW_MAX];
    int nOut = 0, nIn = 0;
    splitShadows(shadows, n, out, nOut, in, nIn);
    (void)in;
    (void)nIn;
    if (nOut <= 0)
    {
        return;
    }
    float l, t, rr, b;
    morph::shadowOutsetExtent(out, nOut, l, t, rr, b);
    // 1px AA margin so soft edges never clip.
    l += 1.0f;
    t += 1.0f;
    rr += 1.0f;
    b += 1.0f;
    float sxy[2] = {bx, by};
    float swh[2] = {bw, bh};
    r.drawBoxShadow(bx - l, by - t, bw + l + rr, bh + t + b, sxy, swh,
                    radii, out, nOut, false);
}

// Inset pass over the padding box (px,py,pw,ph) with inner radii: clipped
// to the shape itself, so calling after the border is harmless.
inline void paintInset(Renderer& r, float px, float py, float pw, float ph,
                       const float radii[4], const BoxShadow* shadows, int n)
{
    BoxShadow out[MORPH_SHADOW_MAX];
    BoxShadow in[MORPH_SHADOW_MAX];
    int nOut = 0, nIn = 0;
    splitShadows(shadows, n, out, nOut, in, nIn);
    (void)out;
    (void)nOut;
    if (nIn <= 0)
    {
        return;
    }
    float sxy[2] = {px, py};
    float swh[2] = {pw, ph};
    r.drawBoxShadow(px, py, pw, ph, sxy, swh, radii, in, nIn, true);
}

// Per-op state for MorphStyle exec sites (widget display lists): resolve
// once, outset before the fill, inset after (padding-box clipped).
struct ShadowExec
{
    BoxShadow tmp[MORPH_SHADOW_MAX];
    int n = 0;
};

inline void shadowPre(Renderer& r, const MorphStyle& style, const UnitEnv& env,
                      float bx, float by, float bw, float bh,
                      const float radii[4], ShadowExec& e)
{
    e.n = style.boxShadowSet ? morph::resolveBoxShadows(style, env, e.tmp) : 0;
    if (e.n > 0)
    {
        paintOutset(r, bx, by, bw, bh, radii, e.tmp, e.n);
    }
}

inline void shadowPost(Renderer& r, float px, float py, float pw, float ph,
                       const float radii[4], ShadowExec& e)
{
    if (e.n > 0)
    {
        paintInset(r, px, py, pw, ph, radii, e.tmp, e.n);
    }
}

} // namespace shadow_paint

#endif
