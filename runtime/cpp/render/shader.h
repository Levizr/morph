#pragma once
#include "vendor/glad/glad.h"
#include <cstdio>

// ── GLSL shader sources ──────────────────────────────────────
// The transform feature adds a per-instance model matrix (mat4 attribute).
// Raw string literals can't contain #ifdef, so splice with adjacent literals.

#ifdef MORPH_FEATURE_TRANSFORM
#define MORPH_GLSL_MODEL_DECL "layout(location = 7) in mat4 aModel;\n"
#define MORPH_GLSL_MODEL_MUL "aModel * "
#else
#define MORPH_GLSL_MODEL_DECL ""
#define MORPH_GLSL_MODEL_MUL ""
#endif

static const char* kQuadVertSrc = R"glsl(
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec4 aInst0;
layout(location = 2) in vec4 aInst1;
layout(location = 3) in vec4 aRadii;
layout(location = 4) in float aBorderWidth;
layout(location = 5) in vec4 aBorderColor;
layout(location = 6) in float aBorderOnly;
layout(location = 11) in float aBorderMode;
layout(location = 12) in vec2 aBorderAdj;
)glsl" MORPH_GLSL_MODEL_DECL R"glsl(
uniform mat4 uProj;
out vec4 vColor;
out vec2 vUV;
out vec2 vSize;
out vec4 vRadii;
out float vBorderWidth;
out vec4 vBorderColor;
out float vBorderOnly;
out float vBorderMode;
out vec2 vBorderAdj;
void main() {
    vec2 pos = aInst0.xy + aPos * aInst0.zw;
    gl_Position = uProj * )glsl" MORPH_GLSL_MODEL_MUL R"glsl(vec4(pos, 0.0, 1.0);
    vColor = aInst1;
    vUV = aPos;
    vSize = aInst0.zw;
    vRadii = aRadii;
    vBorderWidth = aBorderWidth;
    vBorderColor = aBorderColor;
    vBorderOnly = aBorderOnly;
    vBorderMode = aBorderMode;
    vBorderAdj = aBorderAdj;
}
)glsl";

static const char* kQuadFragSrc = R"glsl(
#version 330 core
in vec4 vColor;
in vec2 vUV;
in vec2 vSize;
in vec4 vRadii;
in float vBorderWidth;
in vec4 vBorderColor;
in float vBorderOnly;
in float vBorderMode;
in vec2 vBorderAdj;
uniform bool uStencilMode;
out vec4 FragColor;

// Precise Signed Distance Field for rounded rectangles
float sdRoundedBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

// Per-corner radii in CSS order (TL TR BR BL), fitted to the box per
// the border-radius scale-down rule: radii are clamped to half the
// box, and when adjacent radii overflow an edge every radius scales by
// the tightest edge ratio.
vec4 fitRadii(vec4 r, vec2 halfSize) {
    vec2 full = halfSize * 2.0;
    float s = min(min(full.x / max(r.x + r.y, 1e-4), full.x / max(r.z + r.w, 1e-4)),
                  min(full.y / max(r.x + r.w, 1e-4), full.y / max(r.y + r.z, 1e-4)));
    // Scale FIRST, then clamp: clamping first would shrink a huge radius
    // (pill 9999px) to half the box and then scale it to ~0 (sharp).
    return max(min(r * min(s, 1.0), min(halfSize.x, halfSize.y)), 0.0);
}

// Radius for this fragment's quadrant. p is box-centered with the top
// band at p.y < 0 (see borderSideMask), so TL is (-x, -y).
float pickRadius(vec2 p, vec4 r) {
    float top = p.x > 0.0 ? r.y : r.x;
    float bot = p.x > 0.0 ? r.z : r.w;
    return p.y > 0.0 ? bot : top;
}

// 1 inside the corner square spanned by outer corner o and true inner
// corner i and on the wanted side of the o->i diagonal (CSS miter);
// 1 outside the square so straight edges pass through untouched. The
// diagonal test is smoothed over aa (same SDF fringe as the ring).
// Same-color joints pass aa = 0 for a hard binary step: both sides
// paint the same color so the boundary is invisible either way, and a
// binary assignment keeps full coverage on exactly one side. Any
// smoothing here (even 1e-4) leaves the diagonal pixels at ~0.5 from
// each side, which composites to 0.75 coverage and reads as a dark
// notch (probed on dashed corners vs Chrome).
float cornerKeep(vec2 q, vec2 o, vec2 i, float keepPositive, float aa) {
    vec2 lo = min(o, i);
    vec2 hi = max(o, i);
    float inside = step(lo.x, q.x) * step(q.x, hi.x) *
                   step(lo.y, q.y) * step(q.y, hi.y);
    vec2 v = i - o;
    vec2 w = q - o;
    float cross = v.x * w.y - v.y * w.x;
    float dist = cross / max(length(v), 1e-4);
    float kept;
    if (aa == 0.0) {
        kept = keepPositive > 0.5 ? step(0.0, dist) : 1.0 - step(0.0, dist);
    } else {
        kept = keepPositive > 0.5 ? smoothstep(-aa, aa, dist)
                                  : 1.0 - smoothstep(-aa, aa, dist);
    }
    return mix(1.0, kept, inside);
}

// Border ring side mask. Side follows the ring mode: 0 = full ring,
// 1 = top, 2 = right, 3 = bottom, 4 = left. bw is this side's own
// width; adj carries the two adjacent side widths (pattern-start
// corner first); radii holds the box corner radii (TL TR BR BL).
// Each corner splits along the diagonal from the outer corner toward
// the inner corner, matching the CSS per-side miter when neighboring
// widths differ. Rounded corners extend the split box to at least the
// radius: the arc band swings up to 0.3R outside a bw strip, and a
// bw-sized box would leave a background wedge both sides reject.
// Sharp corners keep today's exact geometry (extension is a no-op).
float borderSideMask(vec2 p, vec2 halfSize, float bw, float side, vec2 adj, float aa,
                     float joint, vec4 radii) {
    if (side < 0.5) {
        return 1.0;
    }
    float hx = halfSize.x;
    float hy = halfSize.y;
    // Every side keeps the interior side of its corner diagonals:
    // keepStart = 0 (drop cross > 0) at the pattern-start corner,
    // keepEnd = 1 at the other. The o->i diagonal runs outer->inner in
    // a different screen direction per corner, but the side's own
    // territory is always on the cross <= 0 side at the start corner
    // and cross >= 0 at the end corner (verified against Chrome for all
    // four joints). An alternating parity here drops the same triangle
    // from both neighbors and leaves a background-colored gap.
    float keepStart = 0.0;
    float keepEnd = 1.0;
    // Joint codes from the mode fraction, packed CPU-side as
    // (start + 4 * end) / 16 — 0 = AA miter (different colors), 1 = butt
    // (identical solids: skip the diagonal, overlap stays crisp),
    // 2 = hard miter (same color: exact tiling, no seam). Exact in
    // float32, so integer compares are safe.
    float jointCode = joint * 16.0;
    float startCode = mod(jointCode, 4.0);
    float endCode = floor(jointCode / 4.0);
    bool buttStart = startCode == 1.0;
    bool buttEnd = endCode == 1.0;
    float aaStart = startCode == 2.0 ? 0.0 : aa;
    float aaEnd = endCode == 2.0 ? 0.0 : aa;
    if (side < 1.5) {
        vec2 oS = vec2(-hx, -hy);
        vec2 oE = vec2(hx, -hy);
        vec2 dS = vec2(adj.x, bw);
        vec2 dE = vec2(-adj.y, bw);
        vec2 ieS = oS + sign(dS) * max(abs(dS), radii.xx);
        vec2 ieE = oE + sign(dE) * max(abs(dE), radii.yy);
        float wS = max(abs(dS.x), radii.x);
        float wE = max(abs(dE.x), radii.y);
        float hS = max(abs(dS.y), radii.x);
        float hE = max(abs(dE.y), radii.y);
        float h = (p.x < oS.x + wS) ? hS : ((p.x > oE.x - wE) ? hE : bw);
        float mask = step(p.y, -hy + h);
        if (!buttStart) {
            mask *= cornerKeep(p, oS, ieS, keepStart, aaStart);
        }
        if (!buttEnd) {
            mask *= cornerKeep(p, oE, ieE, keepEnd, aaEnd);
        }
        return mask;
    } else if (side < 2.5) {
        vec2 oS = vec2(hx, -hy);
        vec2 oE = vec2(hx, hy);
        vec2 dS = vec2(-bw, adj.x);
        vec2 dE = vec2(-bw, -adj.y);
        vec2 ieS = oS + sign(dS) * max(abs(dS), radii.yy);
        vec2 ieE = oE + sign(dE) * max(abs(dE), radii.zz);
        float wS = max(abs(dS.x), radii.y);
        float wE = max(abs(dE.x), radii.z);
        float hS = max(abs(dS.y), radii.y);
        float hE = max(abs(dE.y), radii.z);
        float w = (p.y < oS.y + hS) ? wS : ((p.y > oE.y - hE) ? wE : bw);
        float mask = step(hx - w, p.x);
        if (!buttStart) {
            mask *= cornerKeep(p, oS, ieS, keepStart, aaStart);
        }
        if (!buttEnd) {
            mask *= cornerKeep(p, oE, ieE, keepEnd, aaEnd);
        }
        return mask;
    } else if (side < 3.5) {
        vec2 oS = vec2(hx, hy);
        vec2 oE = vec2(-hx, hy);
        vec2 dS = vec2(-adj.x, -bw);
        vec2 dE = vec2(adj.y, -bw);
        vec2 ieS = oS + sign(dS) * max(abs(dS), radii.zz);
        vec2 ieE = oE + sign(dE) * max(abs(dE), radii.ww);
        float wS = max(abs(dS.x), radii.z);
        float wE = max(abs(dE.x), radii.w);
        float hS = max(abs(dS.y), radii.z);
        float hE = max(abs(dE.y), radii.w);
        float h = (p.x > oS.x - wS) ? hS : ((p.x < oE.x + wE) ? hE : bw);
        float mask = step(hy - h, p.y);
        if (!buttStart) {
            mask *= cornerKeep(p, oS, ieS, keepStart, aaStart);
        }
        if (!buttEnd) {
            mask *= cornerKeep(p, oE, ieE, keepEnd, aaEnd);
        }
        return mask;
    }
    vec2 oS = vec2(-hx, hy);
    vec2 oE = vec2(-hx, -hy);
    vec2 dS = vec2(bw, -adj.x);
    vec2 dE = vec2(bw, adj.y);
    vec2 ieS = oS + sign(dS) * max(abs(dS), radii.ww);
    vec2 ieE = oE + sign(dE) * max(abs(dE), radii.xx);
    float wS = max(abs(dS.x), radii.w);
    float wE = max(abs(dE.x), radii.x);
    float hS = max(abs(dS.y), radii.w);
    float hE = max(abs(dE.y), radii.x);
    float w = (p.y > oS.y - hS) ? wS : ((p.y < oE.y + hE) ? wE : bw);
    float mask = step(p.x, -hx + w);
    if (!buttStart) {
        mask *= cornerKeep(p, oS, ieS, keepStart, aaStart);
    }
    if (!buttEnd) {
        mask *= cornerKeep(p, oE, ieE, keepEnd, aaEnd);
    }
    return mask;
}

// Fitted period for a border motif run: Chrome keeps the motif size
// fixed (8px dashes, round dots) and flexes the spacing so a motif
// lands on BOTH corners (first dash starts at s = 0, last ends at
// s = L; first dot centers at bw/2 and L - bw/2). Probed: 759px edge
// runs period 11.94, 58px edge runs 12.5, dashed and dotted alike.
// A strict restart would leave a gap clipped against the far corner.
float fitPeriod(float sideLen, float motif, float nominal) {
    float span = sideLen - motif;
    if (span <= 0.0) {
        return nominal;
    }
    float n = max(floor(span / nominal + 0.5), 1.0);
    return span / n;
}

// Dotted/dashed coverage for a ring band. s is the contour coordinate in
// px from the side's start corner, t2 the transverse coordinate in
// px (0 at band middle, +/-bw/2 at the edges), bw the border width,
// sideLen the side's length in px and aa the edge softness.
// style: 2 = dotted, 3 = dashed.
float borderPattern(float s, float t2, float bw, float style, float aa, float sideLen) {
    if (style < 1.5) {
        return 1.0;
    }
    if (style < 2.5) {
        // Dotted: round dots, diameter bw + 1 (radius measured 2.5px at
        // bw = 4 in Chrome), first dot center bw/2 from the run start
        // like Chrome (2px at bw = 4; 4px at bw = 8), pitch fitted so
        // the last dot centers at L - bw/2 instead of stranding a gap
        // at the far corner. Symmetric fringe (full to R - aa, gone by
        // R + aa) matches Chrome's wide soft falloff; the ring band
        // clips the fringe past the band edges.
        float pitch = fitPeriod(sideLen, bw, bw * 2.0);
        float start = bw * 0.5;
        float k = floor((s - start) / pitch + 0.5);
        float along = s - (start + k * pitch);
        float d = length(vec2(along, t2));
        float r = bw * 0.5 + 0.5;
        float e = max(aa, 1.0);
        return 1.0 - smoothstep(r - e, r + e, d);
    }
    // Dashed: 2:1 dash:gap over a nominal 3 * bw period (Chrome's
    // ratio), dash length fixed, period fitted per side so the first
    // dash starts at the corner and the last ends at it.
    float dashLen = bw * 2.0;
    if (sideLen > dashLen) {
        float p = fitPeriod(sideLen, dashLen, bw * 3.0);
        float sp = fract(s / p) * p;
        float edge = fwidth(sp) * 1.0 + 1e-4;
        return 1.0 - smoothstep(dashLen - edge, dashLen + edge, sp);
    }
    return 1.0;
}

// Arc-length of the half corner a side owns (territories split at the
// arc midpoint): half of the centerline quarter-arc. Dots ride the band
// middle, so the radius is R - bw/2, clamped at 0 (sharp corners and
// tiny radii contribute nothing and the path reduces to the straight
// tangent-to-tangent run).
float arcHalf(float r, float bw) {
    return 0.7853981634 * max(r - bw * 0.5, 0.0);
}

// Pattern coordinate in px along a side's path, restarting at every
// side so dashes never wrap around corners L-shaped (browsers restart
// the dash run per side). Side follows the ring mode: 0 = nearest edge
// (full ring), 1 = top, 2 = right, 3 = bottom, 4 = left. Each side
// starts at its clockwise start corner, except the left which runs
// top-down (s = 0 at the top corner): bottom-up phasing leaves corner
// dots/dashes to box height modulo the period, opening cracks.
// Rounded corners contribute arc length around the corner up to the
// arc midpoint (where side territories split): a straight-line
// coordinate compresses spacing on the arc and splits dots at the
// joint diagonal, cutting them. With arc length, dots stay round and
// evenly spaced around the bend (probed in Chrome: uniform 8px pitch
// at bw = 4 straight through 40px arcs). Sharp corners keep today's
// exact straight coordinate (arcs are zero).
float borderSideCoord(vec2 p, vec2 halfSize, float side, vec4 radii, float bw) {
    float hx = halfSize.x;
    float hy = halfSize.y;
    if (side > 0.5 && side < 1.5) {
        // Top runs left to right: TL half-arc, straight, TR half-arc.
        float aS = arcHalf(radii.x, bw);
        if (radii.x > 0.0 && p.x < -hx + radii.x && p.y < -hy + radii.x) {
            float ang = atan(p.y - (-hy + radii.x), p.x - (-hx + radii.x));
            return clamp(ang + 2.35619449, 0.0, 0.78539816) * max(radii.x - bw * 0.5, 0.0);
        }
        if (radii.y > 0.0 && p.x > hx - radii.y && p.y < -hy + radii.y) {
            float ang = atan(p.y - (-hy + radii.y), p.x - (hx - radii.y));
            float straight = max(hx * 2.0 - radii.x - radii.y, 0.0);
            return aS + straight + clamp(ang + 1.57079633, 0.0, 0.78539816) * max(radii.y - bw * 0.5, 0.0);
        }
        return aS + (p.x - (-hx + radii.x));
    }
    if (side > 1.5 && side < 2.5) {
        // Right runs top to bottom: TR half-arc, straight, BR half-arc.
        float aS = arcHalf(radii.y, bw);
        if (radii.y > 0.0 && p.x > hx - radii.y && p.y < -hy + radii.y) {
            float ang = atan(p.y - (-hy + radii.y), p.x - (hx - radii.y));
            return clamp(ang + 0.78539816, 0.0, 0.78539816) * max(radii.y - bw * 0.5, 0.0);
        }
        if (radii.z > 0.0 && p.x > hx - radii.z && p.y > hy - radii.z) {
            float ang = atan(p.y - (hy - radii.z), p.x - (hx - radii.z));
            float straight = max(hy * 2.0 - radii.y - radii.z, 0.0);
            return aS + straight + clamp(ang, 0.0, 0.78539816) * max(radii.z - bw * 0.5, 0.0);
        }
        return aS + (p.y - (-hy + radii.y));
    }
    if (side > 2.5 && side < 3.5) {
        // Bottom runs right to left: BR half-arc, straight, BL half-arc.
        float aS = arcHalf(radii.z, bw);
        if (radii.z > 0.0 && p.x > hx - radii.z && p.y > hy - radii.z) {
            float ang = atan(p.y - (hy - radii.z), p.x - (hx - radii.z));
            return clamp(ang - 0.78539816, 0.0, 0.78539816) * max(radii.z - bw * 0.5, 0.0);
        }
        if (radii.w > 0.0 && p.x < -hx + radii.w && p.y > hy - radii.w) {
            float ang = atan(p.y - (hy - radii.w), p.x - (-hx + radii.w));
            float straight = max(hx * 2.0 - radii.z - radii.w, 0.0);
            return aS + straight + clamp(ang - 1.57079633, 0.0, 0.78539816) * max(radii.w - bw * 0.5, 0.0);
        }
        return aS + ((hx - radii.z) - p.x);
    }
    if (side > 3.5) {
        // Left runs top-down (s = 0 at the top corner): TL half-arc,
        // straight, BL half-arc.
        float aS = arcHalf(radii.x, bw);
        if (radii.x > 0.0 && p.x < -hx + radii.x && p.y < -hy + radii.x) {
            float ang = atan(p.y - (-hy + radii.x), p.x - (-hx + radii.x));
            if (ang > 0.0) {
                ang -= 6.28318531;
            }
            return clamp(-2.35619449 - ang, 0.0, 0.78539816) * max(radii.x - bw * 0.5, 0.0);
        }
        if (radii.w > 0.0 && p.x < -hx + radii.w && p.y > hy - radii.w) {
            float ang = atan(p.y - (hy - radii.w), p.x - (-hx + radii.w));
            float straight = max(hy * 2.0 - radii.x - radii.w, 0.0);
            return aS + straight + clamp(3.14159265 - ang, 0.0, 0.78539816) * max(radii.w - bw * 0.5, 0.0);
        }
        return aS + (p.y - (-hy + radii.x));
    }
    float dT = p.y + hy;
    float dB = hy - p.y;
    float dL = p.x + hx;
    float dR = hx - p.x;
    float m = min(min(dT, dB), min(dL, dR));
    if (m == dT) {
        return p.x + hx;
    } else if (m == dR) {
        return p.y + hy;
    } else if (m == dB) {
        return hx - p.x;
    }
    return hy - p.y;
}

void main() {
    vec2 halfSize = vSize * 0.5;
    vec2 p = vUV * vSize - halfSize;
    
    // Fitted per-corner radii (clamped + CSS scale-down); the inner
    // ring derives per corner by border width, matching CSS.
    vec4 fitR = fitRadii(vRadii, halfSize);
    float rad = pickRadius(p, fitR);

    // 1. Calculate Exact Outer Signed Distance
    float dist_outer = sdRoundedBox(p, halfSize, rad);
    
    // BROWSER FIX: Compute axis-aligned directional derivatives explicitly
    // This prevents fwidth() from merging X and Y rate-of-change variances 
    vec2 dDist = vec2(dFdx(dist_outer), dFdy(dist_outer));
    float edgeSoftness = length(dDist) * 0.70710678118; // Exact pixel corner scale invariant factor
    
    // Symmetrical 1-pixel wide screenspace coverage transition
    float alpha_outer = 1.0 - smoothstep(-edgeSoftness, edgeSoftness, dist_outer);

    // Stencil optimization mode
    if (uStencilMode) {
        if (alpha_outer < 0.5) discard;
        FragColor = vec4(1.0);
        return;
    }

    // Early discard optimization for completely transparent pixels
    if (alpha_outer < 0.001) discard;

    vec4 color;
    if (vBorderWidth > 0.0) {
        // Calculate Exact Inner Signed Distance (nested concentric layout matching CSS rules)
        vec2 innerHalfSize = halfSize - vBorderWidth;
        vec4 fitRIn = max(fitR - vBorderWidth, 0.0);
        float innerRad = pickRadius(p, fitRIn);

        float dist_inner = sdRoundedBox(p, innerHalfSize, innerRad);
        float alpha_inner = 1.0 - smoothstep(-edgeSoftness, edgeSoftness, dist_inner);

        // Per-side subset (0 = full ring) and dotted/dashed pattern.
        // Mode packs side * 16 + BorderStyle + joint fraction
        // ((start + 4 * end) / 16: 0 = AA miter, 1 = butt, 2 = hard);
        // only dotted/dashed alter coverage, every other style renders
        // solid.
        float bSide = floor(vBorderMode / 16.0);
        float bStyleRaw = vBorderMode - bSide * 16.0;
        // The mode fraction carries butt-joint flags (exact quarters);
        // floor it off so style range checks below see the integer
        // style only.
        float joint = bStyleRaw - floor(bStyleRaw);
        float bStyle = floor(bStyleRaw);
        // Joint AA floor: edgeSoftness is derivative-based and collapses
        // to 0 exactly on the max() crease the joint diagonal follows
        // (probed: aa reads 0 all along the joint ray), which would
        // alias every miter. 0.7 is the nominal 1px SDF fringe.
        float ringMask = borderSideMask(p, halfSize, vBorderWidth, bSide, vBorderAdj,
                                        max(edgeSoftness, 0.7), joint, fitR);
        if (bStyle > 1.5 && bStyle < 3.5) {
            float s = borderSideCoord(p, halfSize, bSide, fitR, vBorderWidth);
            float t2 = (dist_outer + dist_inner) * 0.5;
            // Path length for motif fitting: the straight
            // tangent-to-tangent run plus the two owned half-arcs
            // (territories split at arc midpoints). Sharp corners
            // contribute zero and this reduces to today's full side.
            // Mode 0 (full ring) only carries solid, so the zeros there
            // just select the strict phase.
            float rS = 0.0;
            float rE = 0.0;
            float fullLen = 0.0;
            if (bSide > 0.5 && bSide < 1.5) {
                rS = fitR.x;
                rE = fitR.y;
                fullLen = halfSize.x * 2.0;
            } else if (bSide > 1.5 && bSide < 2.5) {
                rS = fitR.y;
                rE = fitR.z;
                fullLen = halfSize.y * 2.0;
            } else if (bSide > 2.5 && bSide < 3.5) {
                rS = fitR.z;
                rE = fitR.w;
                fullLen = halfSize.x * 2.0;
            } else if (bSide > 3.5) {
                rS = fitR.x;
                rE = fitR.w;
                fullLen = halfSize.y * 2.0;
            }
            float pathLen = max(fullLen - rS - rE, 0.0) + arcHalf(rS, vBorderWidth) + arcHalf(rE, vBorderWidth);
            float fitLen = pathLen;
            if (bStyle > 1.5 && bStyle < 2.5) {
                // Dotted: a rounded end (radius past the border width)
                // straddles the midpoint joint with a half-pitch (bw)
                // inset, so no dot center lands near the split diagonal
                // and none is ever cut; a sharp end keeps today's bw/2
                // anchor (dots merge at sharp corners like Chrome).
                float insS = rS > vBorderWidth ? vBorderWidth : vBorderWidth * 0.5;
                float insE = rE > vBorderWidth ? vBorderWidth : vBorderWidth * 0.5;
                s -= insS - vBorderWidth * 0.5;
                fitLen = pathLen - (insS + insE) + vBorderWidth;
            } else {
                // Dashed: without an inset both neighbors end/start a
                // dash exactly at the shared midpoint, fusing into a
                // double-length dash with no gap. A rounded end insets
                // the run by half a gap (bw/2) so a nominal gap
                // straddles the joint; sharp ends stay corner-anchored
                // exactly like today.
                float insS = rS > vBorderWidth ? vBorderWidth * 0.5 : 0.0;
                float insE = rE > vBorderWidth ? vBorderWidth * 0.5 : 0.0;
                s -= insS;
                fitLen = pathLen - (insS + insE);
            }
            ringMask *= borderPattern(s, t2, vBorderWidth, bStyle, edgeSoftness, fitLen);
        }

        if (vBorderOnly > 0.5) {
            // Border-only: Smoothly strip out the interior
            float ringAlpha = alpha_outer * (1.0 - alpha_inner) * ringMask;
            color = vec4(vBorderColor.rgb, vBorderColor.a * ringAlpha);
        } else {
            // Fill + Border: Multi-layered destination alpha blending
            vec4 interiorFill = vec4(vColor.rgb, vColor.a * alpha_inner);
            vec4 borderLayer = vec4(vBorderColor.rgb, vBorderColor.a * ringMask);

            color = mix(interiorFill, borderLayer, borderLayer.a * (1.0 - alpha_inner));
            color.a *= alpha_outer;
        }
    } else {
        color = vec4(vColor.rgb, vColor.a * alpha_outer);
    }

    FragColor = color;
}
)glsl";



#ifdef MORPH_FEATURE_TEXT
static const char* kTextVertSrc = R"glsl(
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec4 aInst0;
layout(location = 2) in vec4 aInst1;
layout(location = 3) in vec4 aInst2;
layout(location = 4) in float aIsColor;
)glsl" MORPH_GLSL_MODEL_DECL R"glsl(
uniform mat4 uProj;
out vec4 vColor;
out vec2 vUV;
flat out float vIsColor;
void main() {
    vec2 pos = aInst0.xy + aPos * aInst0.zw;
    gl_Position = uProj * )glsl" MORPH_GLSL_MODEL_MUL R"glsl(vec4(pos, 0.0, 1.0);
    vUV = mix(aInst1.xy, aInst1.zw, aPos);
    vColor = aInst2;
    vIsColor = aIsColor;
}
)glsl";

static const char* kTextFragSrc = R"glsl(
#version 330 core
in vec4 vColor;
in vec2 vUV;
flat in float vIsColor;
uniform sampler2D uAtlas;
uniform sampler2D uColorAtlas;
out vec4 FragColor;
void main() {
    if (vIsColor > 0.5) {
        vec4 c = texture(uColorAtlas, vUV);
        FragColor = vec4(c.rgb, c.a * vColor.a);
    } else {
        float alpha = texture(uAtlas, vUV).r;
        FragColor = vec4(vColor.rgb, vColor.a * alpha);
    }
}
)glsl";
#endif

#ifdef MORPH_FEATURE_IMAGE
static const char* kImageVertSrc = R"glsl(
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec4 aInst0;
layout(location = 2) in vec4 aInst1;
layout(location = 3) in vec4 aInst2;
)glsl" MORPH_GLSL_MODEL_DECL R"glsl(
uniform mat4 uProj;
out vec2 vUV;
out vec4 vTint;
void main() {
    vec2 pos = aInst0.xy + aPos * aInst0.zw;
    gl_Position = uProj * )glsl" MORPH_GLSL_MODEL_MUL R"glsl(vec4(pos, 0.0, 1.0);
    vUV = mix(aInst1.xy, aInst1.zw, aPos);
    vTint = aInst2;
}
)glsl";

static const char* kImageFragSrc = R"glsl(
#version 330 core
in vec2 vUV;
in vec4 vTint;
uniform sampler2D uTexture;
uniform bool uStencilMode;
out vec4 FragColor;
void main() {
    vec4 texel = texture(uTexture, vUV);
    if (uStencilMode) {
        if (texel.a < 0.5) discard;
        FragColor = vec4(1.0);
        return;
    }
    FragColor = texel * vTint;
}
)glsl";
#endif

#ifdef MORPH_FEATURE_GRADIENT
// ── Gradient fill quad (immediate mode) ──────────────────────
// Same rounded-box SDF as the batched quad shader, but the fill color is
// evaluated per-pixel from linear-gradient stops passed as uniforms (stop
// arrays can't ride the 16-slot instanced-attribute budget, and gradient
// boxes are rare enough that one draw call each is negligible).
static const char* kGradQuadVertSrc = R"glsl(
#version 330 core
layout(location = 0) in vec2 aPos;
uniform mat4 uProj;
uniform mat4 uModel;
uniform vec4 uRect;
out vec2 vUV;
out vec2 vSize;
void main() {
    vec2 pos = uRect.xy + aPos * uRect.zw;
    gl_Position = uProj * uModel * vec4(pos, 0.0, 1.0);
    vUV = aPos;
    vSize = uRect.zw;
}
)glsl";

static const char* kGradQuadFragSrc = R"glsl(
#version 330 core
in vec2 vUV;
in vec2 vSize;
uniform vec2 uGradDir;
uniform int uGradCount;
uniform vec4 uGradColors[8];
uniform float uGradOffsets[8];
uniform int uGradRepeating;
uniform float uGradPeriod;
uniform vec4 uRadii;
uniform float uBorderWidth;
uniform vec4 uBorderColor;
uniform bool uStencilMode;

// Border gradient
uniform int uBorderCount;
uniform vec4 uBorderColors[8];
uniform float uBorderOffsets[8];
uniform int uBorderRepeating;
uniform float uBorderPeriod;
out vec4 FragColor;

float sdRoundedBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

vec4 fitRadii(vec4 r, vec2 halfSize) {
    vec2 full = halfSize * 2.0;
    float s = min(min(full.x / max(r.x + r.y, 1e-4), full.x / max(r.z + r.w, 1e-4)),
                  min(full.y / max(r.x + r.w, 1e-4), full.y / max(r.y + r.z, 1e-4)));
    // Scale FIRST, then clamp: clamping first would shrink a huge radius
    // (pill 9999px) to half the box and then scale it to ~0 (sharp).
    return max(min(r * min(s, 1.0), min(halfSize.x, halfSize.y)), 0.0);
}

float pickRadius(vec2 p, vec4 r) {
    float top = p.x > 0.0 ? r.y : r.x;
    float bot = p.x > 0.0 ? r.z : r.w;
    return p.y > 0.0 ? bot : top;
}

// Stop weight with explicit hard-stop stepping. Double positions share an
// offset, so lerping across the degenerate span would divide by ~0 and
// amplify rounding noise into speckle — step instead.
float gradStopF(float t, float prevO, float curO) {
    float span = curO - prevO;
    return (span <= 1e-6) ? ((t < curO) ? 0.0 : 1.0)
                          : clamp((t - prevO) / span, 0.0, 1.0);
}

void main() {
    vec2 halfSize = vSize * 0.5;
    vec2 p = vUV * vSize - halfSize;

    vec4 fitR = fitRadii(uRadii, halfSize);
    float rad = pickRadius(p, fitR);
    float dist_outer = sdRoundedBox(p, halfSize, rad);

    vec2 dDist = vec2(dFdx(dist_outer), dFdy(dist_outer));
    float edgeSoftness = length(dDist) * 0.70710678118;

    float alpha_outer = 1.0 - smoothstep(-edgeSoftness, edgeSoftness, dist_outer);

    if (uStencilMode) {
        if (alpha_outer < 0.5) discard;
        FragColor = vec4(1.0);
        return;
    }

    if (alpha_outer < 0.001) discard;

    // Gradient position along the axis, 0..1 across the gradient line.
    float projLen = abs(vSize.x * uGradDir.x) + abs(vSize.y * uGradDir.y);
    projLen = max(projLen, 1e-4);
    float t = dot(p, uGradDir) / projLen + 0.5;

    if (uGradRepeating > 0 && uGradCount >= 2) {
        float o0 = uGradOffsets[0];
        if (uGradPeriod > 1e-6) {
            float rel = (t - o0) / uGradPeriod;
            t = o0 + (rel - floor(rel)) * uGradPeriod;
        } else {
            t = o0;
        }
    }

    // Unrolled with constant indices and uniform guards: dynamic indexing
    // of uniform arrays miscompiles on some drivers (checkerboard
    // speckle), so never index uGradColors/uGradOffsets by a variable.
    vec4 g = uGradColors[0];
    if (uGradCount > 1)
        g = mix(g, uGradColors[1], gradStopF(t, uGradOffsets[0], uGradOffsets[1]));
    if (uGradCount > 2)
        g = mix(g, uGradColors[2], gradStopF(t, uGradOffsets[1], uGradOffsets[2]));
    if (uGradCount > 3)
        g = mix(g, uGradColors[3], gradStopF(t, uGradOffsets[2], uGradOffsets[3]));
    if (uGradCount > 4)
        g = mix(g, uGradColors[4], gradStopF(t, uGradOffsets[3], uGradOffsets[4]));
    if (uGradCount > 5)
        g = mix(g, uGradColors[5], gradStopF(t, uGradOffsets[4], uGradOffsets[5]));
    if (uGradCount > 6)
        g = mix(g, uGradColors[6], gradStopF(t, uGradOffsets[5], uGradOffsets[6]));
    if (uGradCount > 7)
        g = mix(g, uGradColors[7], gradStopF(t, uGradOffsets[6], uGradOffsets[7]));

    vec4 color;
    if (uBorderWidth > 0.0) {
        vec2 innerHalfSize = halfSize - uBorderWidth;
        float innerRad = pickRadius(p, max(fitR - uBorderWidth, 0.0));

        float dist_inner = sdRoundedBox(p, innerHalfSize, innerRad);
        float alpha_inner = 1.0 - smoothstep(-edgeSoftness, edgeSoftness, dist_inner);

        // Border gradient: evaluate if uBorderCount > 0. Unrolled with
        // constant indices like the fill above: dynamic indexing of
        // uniform arrays miscompiles on some drivers.
        vec4 borderColorVal;
        if (uBorderCount > 0) {
            // Evaluate border gradient at same parameter t
            vec4 bg = uBorderColors[0];
            if (uBorderCount > 1)
                bg = mix(bg, uBorderColors[1], gradStopF(t, uBorderOffsets[0], uBorderOffsets[1]));
            if (uBorderCount > 2)
                bg = mix(bg, uBorderColors[2], gradStopF(t, uBorderOffsets[1], uBorderOffsets[2]));
            if (uBorderCount > 3)
                bg = mix(bg, uBorderColors[3], gradStopF(t, uBorderOffsets[2], uBorderOffsets[3]));
            if (uBorderCount > 4)
                bg = mix(bg, uBorderColors[4], gradStopF(t, uBorderOffsets[3], uBorderOffsets[4]));
            if (uBorderCount > 5)
                bg = mix(bg, uBorderColors[5], gradStopF(t, uBorderOffsets[4], uBorderOffsets[5]));
            if (uBorderCount > 6)
                bg = mix(bg, uBorderColors[6], gradStopF(t, uBorderOffsets[5], uBorderOffsets[6]));
            if (uBorderCount > 7)
                bg = mix(bg, uBorderColors[7], gradStopF(t, uBorderOffsets[6], uBorderOffsets[7]));
            borderColorVal = bg;
        } else {
            borderColorVal = uBorderColor;
        }

        vec4 interiorFill = vec4(g.rgb, g.a * alpha_inner);
        vec4 borderLayer = vec4(borderColorVal.rgb, borderColorVal.a);

        color = mix(interiorFill, borderLayer, borderLayer.a * (1.0 - alpha_inner));
        color.a *= alpha_outer;
    } else {
        color = vec4(g.rgb, g.a * alpha_outer);
    }

    FragColor = color;
}
)glsl";
#endif

// ── Shared unit quad ─────────────────────────────────────────

static const float kQuadVerts[] = {
    0.0f, 0.0f,
    1.0f, 0.0f,
    1.0f, 1.0f,
    0.0f, 1.0f,
};

static const GLuint kQuadIndices[] = {
    0, 1, 2,
    2, 3, 0,
};

// ── Helpers ──────────────────────────────────────────────────

static GLuint compileShader(GLenum type, const char* src) {
    GLuint s = glCreateShader(type);
    glShaderSource(s, 1, &src, nullptr);
    glCompileShader(s);
    GLint ok = 0;
    glGetShaderiv(s, GL_COMPILE_STATUS, &ok);
    if (!ok) {
        char log[512];
        glGetShaderInfoLog(s, sizeof(log), nullptr, log);
        fprintf(stderr, "[GL] shader compile error:\n%s\n", log);
    }
    return s;
}

static void createProgram(const char* vsSrc, const char* fsSrc,
                          GLuint& prog, GLint& uProj) {
    GLuint vs = compileShader(GL_VERTEX_SHADER, vsSrc);
    GLuint fs = compileShader(GL_FRAGMENT_SHADER, fsSrc);
    prog = glCreateProgram();
    glAttachShader(prog, vs);
    glAttachShader(prog, fs);
    glLinkProgram(prog);
    GLint ok = 0;
    glGetProgramiv(prog, GL_LINK_STATUS, &ok);
    if (!ok) {
        char log[512];
        glGetProgramInfoLog(prog, sizeof(log), nullptr, log);
        fprintf(stderr, "[GL] program link error:\n%s\n", log);
    }
    glDeleteShader(vs);
    glDeleteShader(fs);
    uProj = glGetUniformLocation(prog, "uProj");
}
