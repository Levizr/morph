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
)glsl" MORPH_GLSL_MODEL_DECL R"glsl(
uniform mat4 uProj;
out vec4 vColor;
out vec2 vUV;
out vec2 vSize;
out float vRadius;
out float vBorderWidth;
out vec4 vBorderColor;
out float vBorderOnly;
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
uniform bool uStencilMode;
out vec4 FragColor;

// Precise Signed Distance Field for rounded rectangles
float sdRoundedBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
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

        if (vBorderOnly > 0.5) {
            // Border-only: Smoothly strip out the interior
            float ringAlpha = alpha_outer * (1.0 - alpha_inner);
            color = vec4(vBorderColor.rgb, vBorderColor.a * ringAlpha);
        } else {
            // Fill + Border: Multi-layered destination alpha blending
            vec4 interiorFill = vec4(vColor.rgb, vColor.a * alpha_inner);
            vec4 borderLayer = vec4(vBorderColor.rgb, vBorderColor.a);
            
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
uniform float uRadius;
uniform float uBorderWidth;
uniform vec4 uBorderColor;
uniform bool uStencilMode;
out vec4 FragColor;

float sdRoundedBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
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
        float o1 = uGradOffsets[uGradCount - 1];
        float period = o1 - o0;
        if (abs(period) > 1e-6) {
            t = o0 + mod(t - o0, period);
        } else {
            t = o0;
        }
    }

    vec4 g = uGradColors[0];
    for (int i = 1; i < 8; i++) {
        if (i >= uGradCount) break;
        float span = max(uGradOffsets[i] - uGradOffsets[i - 1], 1e-6);
        float f = clamp((t - uGradOffsets[i - 1]) / span, 0.0, 1.0);
        g = mix(g, uGradColors[i], f);
    }

    vec4 color;
    if (uBorderWidth > 0.0) {
        vec2 innerHalfSize = halfSize - uBorderWidth;
        float innerRad = max(rad - uBorderWidth, 0.0);

        float dist_inner = sdRoundedBox(p, innerHalfSize, innerRad);
        float alpha_inner = 1.0 - smoothstep(-edgeSoftness, edgeSoftness, dist_inner);

        vec4 interiorFill = vec4(g.rgb, g.a * alpha_inner);
        vec4 borderLayer = vec4(uBorderColor.rgb, uBorderColor.a);

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
