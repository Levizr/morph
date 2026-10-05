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
layout(location = 3) in float aRadius;
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
out float vRadius;
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
    vRadius = aRadius;
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
in float vRadius;
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
// corner first) so corner squares split along the diagonal from the
// outer corner to the TRUE inner corner, matching the CSS per-side
// miter when neighboring widths differ.
float borderSideMask(vec2 p, vec2 halfSize, float bw, float side, vec2 adj, float aa,
                     float joint) {
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
        float mask = step(p.y, -hy + bw);
        if (!buttStart) {
            mask *= cornerKeep(p, vec2(-hx, -hy), vec2(-hx + adj.x, -hy + bw), keepStart, aaStart);
        }
        if (!buttEnd) {
            mask *= cornerKeep(p, vec2(hx, -hy), vec2(hx - adj.y, -hy + bw), keepEnd, aaEnd);
        }
        return mask;
    } else if (side < 2.5) {
        float mask = step(hx - bw, p.x);
        if (!buttStart) {
            mask *= cornerKeep(p, vec2(hx, -hy), vec2(hx - bw, -hy + adj.x), keepStart, aaStart);
        }
        if (!buttEnd) {
            mask *= cornerKeep(p, vec2(hx, hy), vec2(hx - bw, hy - adj.y), keepEnd, aaEnd);
        }
        return mask;
    } else if (side < 3.5) {
        float mask = step(hy - bw, p.y);
        if (!buttStart) {
            mask *= cornerKeep(p, vec2(hx, hy), vec2(hx - adj.x, hy - bw), keepStart, aaStart);
        }
        if (!buttEnd) {
            mask *= cornerKeep(p, vec2(-hx, hy), vec2(-hx + adj.y, hy - bw), keepEnd, aaEnd);
        }
        return mask;
    }
    float mask = step(p.x, -hx + bw);
    if (!buttStart) {
        mask *= cornerKeep(p, vec2(-hx, hy), vec2(-hx + bw, hy - adj.x), keepStart, aaStart);
    }
    if (!buttEnd) {
        mask *= cornerKeep(p, vec2(-hx, -hy), vec2(-hx + bw, -hy + adj.y), keepEnd, aaEnd);
    }
    return mask;
}

// Fitted period for a border motif run: Chrome keeps the motif size
// fixed (8px dashes, round dots) and flexes the spacing so a motif
// lands on BOTH corners (first dash starts at s = 0, last ends at
// s = L; first dot centers at 2 and L - 2). Probed: 759px edge runs
// period 11.94, 58px edge runs 12.5, dashed and dotted alike. A strict
// restart would leave a gap clipped against the far corner.
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
        // bw = 4 in Chrome), first dot center 2px from the corner like
        // Chrome, pitch fitted so the last dot centers at L - 2 instead
        // of stranding a gap at the far corner. Symmetric fringe (full
        // to R - aa, gone by R + aa) matches Chrome's wide soft
        // falloff; the ring band clips the fringe past the band edges.
        float pitch = fitPeriod(sideLen, 4.0, bw * 2.0);
        float k = floor((s - 2.0) / pitch + 0.5);
        float along = s - (2.0 + k * pitch);
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

// Pattern coordinate in px along a side, restarting at every corner so
// dashes never wrap around corners L-shaped (browsers restart the dash
// run per side). Side follows the ring mode: 0 = nearest edge (full
// ring), 1 = top, 2 = right, 3 = bottom, 4 = left. Each side starts at
// its clockwise start corner: top runs left to right, right runs top to
// bottom, bottom runs right to left, left runs bottom to top.
float borderSideCoord(vec2 p, vec2 halfSize, float side) {
    float hx = halfSize.x;
    float hy = halfSize.y;
    if (side > 0.5 && side < 1.5) {
        return p.x + hx;
    }
    if (side > 1.5 && side < 2.5) {
        return p.y + hy;
    }
    if (side > 2.5 && side < 3.5) {
        return hx - p.x;
    }
    if (side > 3.5) {
        // Left runs top-down like the right side (s = 0 at the top
        // corner): bottom-up phasing leaves corner dots/dashes to box
        // height modulo the period, opening cracks.
        return p.y + hy;
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
    
    // Clamp radius so it can never exceed half of the dimensions
    float rad = min(max(vRadius, 0.0), min(halfSize.x, halfSize.y));

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
        float innerRad = max(rad - vBorderWidth, 0.0);

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
                                        max(edgeSoftness, 0.7), joint);
        if (bStyle > 1.5 && bStyle < 3.5) {
            float s = borderSideCoord(p, halfSize, bSide);
            float t2 = (dist_outer + dist_inner) * 0.5;
            // Side length for motif fitting: top/bottom span the full
            // width, left/right the full height. Mode 0 (full ring) only
            // carries solid, so 0.0 there just selects the strict phase.
            float sideLen = 0.0;
            if (bSide > 0.5 && bSide < 1.5) {
                sideLen = halfSize.x * 2.0;
            } else if (bSide > 1.5 && bSide < 2.5) {
                sideLen = halfSize.y * 2.0;
            } else if (bSide > 2.5 && bSide < 3.5) {
                sideLen = halfSize.x * 2.0;
            } else if (bSide > 3.5) {
                sideLen = halfSize.y * 2.0;
            }
            ringMask *= borderPattern(s, t2, vBorderWidth, bStyle, edgeSoftness, sideLen);
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
uniform float uRadius;
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

    float rad = min(max(uRadius, 0.0), min(halfSize.x, halfSize.y));
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
        float innerRad = max(rad - uBorderWidth, 0.0);

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
