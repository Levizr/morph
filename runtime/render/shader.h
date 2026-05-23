#pragma once
#include "vendor/glad/glad.h"
#include <cstdio>

// ── GLSL shader sources ──────────────────────────────────────

static const char* kQuadVertSrc = R"glsl(
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec4 aInst0;
layout(location = 2) in vec4 aInst1;
layout(location = 3) in float aRadius;
uniform mat4 uProj;
out vec4 vColor;
out vec2 vUV;
out vec2 vSize;
out float vRadius;
void main() {
    vec2 pos = aInst0.xy + aPos * aInst0.zw;
    gl_Position = uProj * vec4(pos, 0.0, 1.0);
    vColor = aInst1;
    vUV = aPos;
    vSize = aInst0.zw;
    vRadius = aRadius;
}
)glsl";

static const char* kQuadFragSrc = R"glsl(
#version 330 core
in vec4 vColor;
in vec2 vUV;
in vec2 vSize;
in float vRadius;
out vec4 FragColor;
void main() {
    vec2 halfSize = vSize * 0.5;
    vec2 d = abs(vUV * vSize - halfSize) - halfSize + vRadius;
    float dist = length(max(d, 0.0)) - vRadius;
    float alpha = 1.0 - smoothstep(0.0, fwidth(dist), max(dist, 0.0));
    FragColor = vec4(vColor.rgb, vColor.a * alpha);
}
)glsl";

#ifdef MORPH_FEATURE_TEXT
static const char* kTextVertSrc = R"glsl(
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec4 aInst0;
layout(location = 2) in vec4 aInst1;
layout(location = 3) in vec4 aInst2;
uniform mat4 uProj;
out vec4 vColor;
out vec2 vUV;
void main() {
    vec2 pos = aInst0.xy + aPos * aInst0.zw;
    gl_Position = uProj * vec4(pos, 0.0, 1.0);
    vUV = mix(aInst1.xy, aInst1.zw, aPos);
    vColor = aInst2;
}
)glsl";

static const char* kTextFragSrc = R"glsl(
#version 330 core
in vec4 vColor;
in vec2 vUV;
uniform sampler2D uAtlas;
out vec4 FragColor;
void main() {
    float alpha = texture(uAtlas, vUV).r;
    FragColor = vec4(vColor.rgb, vColor.a * alpha);
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
