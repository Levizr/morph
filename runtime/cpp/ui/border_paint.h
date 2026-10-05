#pragma once
// Shared fill + border recording/painting for box nodes (rect, button,
// input). A uniform solid border keeps the single-quad fast path so its
// pixels never change; per-side widths/colors, dotted/dashed and
// border-image gradients become a plain fill plus BorderRing ops (one per
// visible side, or one full-ring op when every side matches).
#include "../core/node.h"
#include "radius.h"

#ifdef MORPH_FEATURE_BORDER
// A border-image gradient overrides the border color source. Ring ops
// recorded for it carry a dummy color; execute paths resolve the live
// gradient from the style instead (gradients never ride inside DrawOp).
inline bool borderImageGradActive(const MorphStyle& style)
{
#ifdef MORPH_FEATURE_GRADIENT
    return style.borderImageEnabled && style.borderImageIsGradient &&
        style.borderGradientSet && style.borderGradient.enabled &&
        style.borderGradient.stopCount >= 2;
#else
    (void)style;
    return false;
#endif
}

// Snapped per-side widths (index 0=top, 1=right, 2=bottom, 3=left).
inline void snappedBorderWidths(const MorphStyle& style, bool snap,
                                float out[4])
{
    float widths[4];
    const float* colors[4];
    float modes[4];
    resolveBorderSides(style, widths, colors, modes);
    for (int side = 0; side < 4; side++)
    {
        out[side] = snap ? snapBorderWidth(widths[side]) : widths[side];
    }
}

// Adjacent side widths for one side's two corners, in pattern-start
// order: top = (left, right), right = (top, bottom),
// bottom = (right, left), left = (bottom, top).
inline void sideAdjacents(const float w[4], int side, float& adjS,
                          float& adjE)
{
    if (side == 0)
    {
        adjS = w[3];
        adjE = w[1];
    }
    else if (side == 1)
    {
        adjS = w[0];
        adjE = w[2];
    }
    else if (side == 2)
    {
        adjS = w[1];
        adjE = w[3];
    }
    else
    {
        adjS = w[2];
        adjE = w[0];
    }
}

// Border box for the ring: same rect as the uniform-solid fast path uses
// (full box under border-box, expanded outward otherwise). Per-side
// widths expand each edge by its own width.
inline void borderRingBox(const MorphStyle& style, float sx, float sy,
                          float sw, float sh, const float w[4],
                          float& bx, float& by, float& bw, float& bh)
{
    bx = sx;
    by = sy;
    bw = sw;
    bh = sh;
    if (style.boxSizing != CSS::BoxSizing::BorderBox)
    {
        bx -= w[3];
        by -= w[0];
        bw += w[3] + w[1];
        bh += w[0] + w[2];
    }
}

// Record the fill op plus border ops for a box. plainRounded selects the
// no-border fill shape (buttons/inputs always round, rects only when
// radius > 0); rad is the caller-computed corner radius.
inline void recordBoxOps(std::vector<DrawOp>& out, MorphStyle& style,
                         float sx, float sy, float sw, float sh, float rad,
                         bool snap, bool plainRounded)
{
    float widths[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    const float* colors[4] = {nullptr, nullptr, nullptr, nullptr};
    float modes[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    int visible = resolveBorderSides(style, widths, colors, modes);
    bool imgGrad = borderImageGradActive(style);
    bool uniformSolid = !imgGrad && visible > 0 &&
        borderSidesUniform(widths, colors, modes) &&
        getBorderStyle(style, 0) == CSS::BorderStyle::Solid;
    if (uniformSolid)
    {
        float bw = snap ? snapBorderWidth(widths[0]) : widths[0];
        float bc[4] = {colors[0][0], colors[0][1], colors[0][2],
                       colors[0][3]};
        DrawOp bg;
        if (style.boxSizing == CSS::BoxSizing::BorderBox)
        {
            bg.setBordered(sx, sy, sw, sh, rad, style.bgColor, bw, bc);
        }
        else
        {
            bg.setBordered(sx - bw, sy - bw, sw + 2.0f * bw, sh + 2.0f * bw,
                           rad, style.bgColor, bw, bc);
        }
        out.push_back(bg);
        return;
    }
    DrawOp fill;
    if (plainRounded || rad > 0.0f)
    {
        fill.setRounded(sx, sy, sw, sh, rad, style.bgColor);
    }
    else
    {
        fill.setRect(sx, sy, sw, sh, style.bgColor);
    }
    out.push_back(fill);
    if (visible <= 0)
    {
        return;
    }
    float w[4];
    snappedBorderWidths(style, snap, w);
    float bx, by, bw, bh;
    borderRingBox(style, sx, sy, sw, sh, w, bx, by, bw, bh);
    if (imgGrad)
    {
        float maxW = w[0];
        for (int side = 1; side < 4; side++)
        {
            if (w[side] > maxW)
            {
                maxW = w[side];
            }
        }
        float dummy[4] = {0.0f, 0.0f, 0.0f, 0.0f};
        DrawOp ring;
        ring.setBorderRing(bx, by, bw, bh, rad, maxW, dummy, 0.0f);
        out.push_back(ring);
        return;
    }
    // Uniform style + color collapses to one full-ring op — but only for
    // solid. Dotted/dashed need per-side ops: Chrome fits each side's
    // motif run to its own length (first motif at the start corner,
    // last at the end), which a single continuous ring coordinate
    // cannot express. Same-color sides then share hard-step joints, so
    // the split is seamless.
    if (borderSidesUniform(widths, colors, modes) &&
        getBorderStyle(style, 0) == CSS::BorderStyle::Solid)
    {
        float fullMode = (float)(int)getBorderStyle(style, 0);
        float bc[4] = {colors[0][0], colors[0][1], colors[0][2],
                       colors[0][3]};
        DrawOp ring;
        ring.setBorderRing(bx, by, bw, bh, rad, w[0], bc, fullMode, w[0],
                           w[0]);
        out.push_back(ring);
        return;
    }
    for (int side = 0; side < 4; side++)
    {
        if (w[side] <= 0.0f ||
            getBorderStyle(style, side) == CSS::BorderStyle::None)
        {
            continue;
        }
        float bc[4] = {colors[side][0], colors[side][1], colors[side][2],
                       colors[side][3]};
        float adjS, adjE;
        sideAdjacents(w, side, adjS, adjE);
        float mode = modes[side];
        int nStart, nEnd;
        sideNeighbors(side, nStart, nEnd);
        mode += borderJointFrac(colors, modes, side, nStart, nEnd);
        DrawOp ring;
        ring.setBorderRing(bx, by, bw, bh, rad, w[side], bc, mode, adjS, adjE);
        out.push_back(ring);
    }
}

// Ring ops for the non-uniform / patterned / border-image cases.
inline void paintBoxRings(Renderer& r, MorphStyle& style,
                          float sx, float sy, float sw, float sh, float rad,
                          bool snap)
{
    float widths[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    const float* colors[4] = {nullptr, nullptr, nullptr, nullptr};
    float modes[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    resolveBorderSides(style, widths, colors, modes);
    float w[4];
    snappedBorderWidths(style, snap, w);
    float bx, by, bw, bh;
    borderRingBox(style, sx, sy, sw, sh, w, bx, by, bw, bh);
#ifdef MORPH_FEATURE_GRADIENT
    if (borderImageGradActive(style))
    {
        float maxW = w[0];
        for (int side = 1; side < 4; side++)
        {
            if (w[side] > maxW)
            {
                maxW = w[side];
            }
        }
        float clear[4] = {0.0f, 0.0f, 0.0f, 0.0f};
        r.drawGradRect(bx, by, bw, bh, rad, clear, nullptr, maxW, nullptr,
                       &style.borderGradient);
        return;
    }
#endif
    // Uniform style + color collapses to one full-ring op — but only for
    // solid. Dotted/dashed need per-side ops: Chrome fits each side's
    // motif run to its own length (first motif at the start corner,
    // last at the end), which a single continuous ring coordinate
    // cannot express. Same-color sides then share hard-step joints, so
    // the split is seamless.
    if (borderSidesUniform(widths, colors, modes) &&
        getBorderStyle(style, 0) == CSS::BorderStyle::Solid)
    {
        float fullMode = (float)(int)getBorderStyle(style, 0);
        float bc[4] = {colors[0][0], colors[0][1], colors[0][2],
                       colors[0][3]};
        r.drawBorderRing(bx, by, bw, bh, rad, w[0], bc, fullMode, w[0],
                         w[0]);
        return;
    }
    for (int side = 0; side < 4; side++)
    {
        if (w[side] <= 0.0f ||
            getBorderStyle(style, side) == CSS::BorderStyle::None)
        {
            continue;
        }
        float bc[4] = {colors[side][0], colors[side][1], colors[side][2],
                       colors[side][3]};
        float adjS, adjE;
        sideAdjacents(w, side, adjS, adjE);
        float mode = modes[side];
        int nStart, nEnd;
        sideNeighbors(side, nStart, nEnd);
        mode += borderJointFrac(colors, modes, side, nStart, nEnd);
        r.drawBorderRing(bx, by, bw, bh, rad, w[side], bc, mode, adjS, adjE);
    }
}

// Immediate-mode twin of recordBoxOps for the legacy draw() path.
inline void paintBoxDirect(Renderer& r, MorphStyle& style,
                           float sx, float sy, float sw, float sh, float rad,
                           bool snap, bool plainRounded)
{
    float widths[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    const float* colors[4] = {nullptr, nullptr, nullptr, nullptr};
    float modes[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    int visible = resolveBorderSides(style, widths, colors, modes);
    bool imgGrad = borderImageGradActive(style);
    bool uniformSolid = !imgGrad && visible > 0 &&
        borderSidesUniform(widths, colors, modes) &&
        getBorderStyle(style, 0) == CSS::BorderStyle::Solid;
#ifdef MORPH_FEATURE_GRADIENT
    if (style.bgGradient.enabled)
    {
        float bw = 0.0f;
        const float* bc = nullptr;
        float gx = sx, gy = sy, gw = sw, gh = sh;
        if (uniformSolid)
        {
            bw = snap ? snapBorderWidth(widths[0]) : widths[0];
            bc = colors[0];
            if (style.boxSizing != CSS::BoxSizing::BorderBox)
            {
                gx -= bw;
                gy -= bw;
                gw += 2.0f * bw;
                gh += 2.0f * bw;
            }
        }
        r.drawGradRect(gx, gy, gw, gh, rad, style.bgColor, &style.bgGradient,
                       bw, bc, nullptr);
        if (!uniformSolid && visible > 0)
        {
            paintBoxRings(r, style, sx, sy, sw, sh, rad, snap);
        }
        return;
    }
#endif
    if (uniformSolid)
    {
        float bw = snap ? snapBorderWidth(widths[0]) : widths[0];
        float bc[4] = {colors[0][0], colors[0][1], colors[0][2],
                       colors[0][3]};
        if (style.boxSizing == CSS::BoxSizing::BorderBox)
        {
            r.drawBorderedRoundedRect(sx, sy, sw, sh, rad, style.bgColor, bw,
                                      bc);
        }
        else
        {
            r.drawBorderedRoundedRect(sx - bw, sy - bw, sw + 2.0f * bw,
                                      sh + 2.0f * bw, rad, style.bgColor, bw,
                                      bc);
        }
        return;
    }
    if (plainRounded || rad > 0.0f)
    {
        r.drawRoundedRect(sx, sy, sw, sh, rad, style.bgColor);
    }
    else
    {
        r.drawRect(sx, sy, sw, sh, style.bgColor);
    }
    if (visible > 0)
    {
        paintBoxRings(r, style, sx, sy, sw, sh, rad, snap);
    }
}

// Execute a recorded BorderRing op: forwards the packed side/style mode
// and resolves border-image gradients against the live style.
inline void execBorderRingOp(Renderer& r, DrawOp& op, MorphStyle& style)
{
#ifdef MORPH_FEATURE_GRADIENT
    if (borderImageGradActive(style))
    {
        float clear[4] = {0.0f, 0.0f, 0.0f, 0.0f};
        r.drawGradRect(op.x, op.y, op.w, op.h, op.data[0], clear, nullptr,
                       op.data[1], nullptr, &style.borderGradient);
        return;
    }
#else
    (void)style;
#endif
    r.drawBorderRing(op.x, op.y, op.w, op.h, op.data[0], op.data[1],
                     (float*)&op.br, op.data[2], op.data[3], op.data[4]);
}

#endif // MORPH_FEATURE_BORDER
