#pragma once
#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>
#include <vector>
#include <cstring>
#include <cstdio>
#include <cmath>
#include <unordered_map>
#include "renderer.h"

#ifdef MORPH_FEATURE_TEXT
#include <ft2build.h>
#include FT_FREETYPE_H
#endif

// ── GLSL shaders ─────────────────────────────────────────────

static const char* kQuadVertSrc = R"glsl(
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec4 aInst0;   // x, y, w, h
layout(location = 2) in vec4 aInst1;   // r, g, b, a
layout(location = 3) in float aRadius; // border-radius

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
// ── Text shader ──────────────────────────────────────────────

static const char* kTextVertSrc = R"glsl(
#version 330 core
layout(location = 0) in vec2 aPos;
layout(location = 1) in vec4 aInst0;   // x, y, w, h
layout(location = 2) in vec4 aInst1;   // u1, v1, u2, v2
layout(location = 3) in vec4 aInst2;   // r, g, b, a

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

// ── Default font path ────────────────────────────────────────

static const char* kDefaultFont     = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
static const char* kDefaultFontBold = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
#endif

// ── Shared unit quad (top-left origin, 0..1) ─────────────────

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

// ── Batch renderer ───────────────────────────────────────────

class GLRenderer : public Renderer {
    struct Instance {
        float x, y, w, h;
        float r, g, b, a;
        float radius;
    };

#ifdef MORPH_FEATURE_TEXT
    struct TextInstance {
        float x, y, w, h;
        float u1, v1, u2, v2;
        float r, g, b, a;
    };

    struct GlyphInfo {
        float ax, ay;      // advance
        float bx, by;      // bearing offset
        float gw, gh;      // glyph pixel size
        float u1, v1, u2, v2; // normalized UV in atlas
    };

    struct FontAtlas {
        GLuint texture = 0;
        int w = 0, h = 0;
        int fontSize = 0;
        std::unordered_map<GLchar, GlyphInfo> glyphs;
    };
#endif

    // Quad batch
    GLuint m_vao = 0, m_vbo = 0, m_ibo = 0, m_instVBO = 0;
    GLuint m_shader = 0;
    GLint m_uProj = -1;
    bool m_ready = false;
    std::vector<Instance> m_batch;

#ifdef MORPH_FEATURE_TEXT
    // Text batch
    GLuint m_textVAO = 0, m_textVBO = 0, m_textIBO = 0, m_textInstVBO = 0;
    GLuint m_textShader = 0;
    GLint m_textUProj = -1;
    GLint m_textUAtlas = -1;
    std::unordered_map<std::string, std::vector<TextInstance>> m_textBatches;
    FT_Library m_ft = nullptr;
    std::unordered_map<std::string, FontAtlas> m_atlases;
#endif

    static GLuint compileShader(GLenum type, const char* src) {
        GLuint s = glCreateShader(type);
        glShaderSource(s, 1, &src, nullptr);
        glCompileShader(s);
        GLint ok = 0;
        glGetShaderiv(s, GL_COMPILE_STATUS, &ok);
        if (!ok) {
            char log[512];
            glGetShaderInfoLog(s, sizeof(log), nullptr, log);
            fprintf(stderr, "[GLRenderer] shader compile error:\n%s\n", log);
        }
        return s;
    }

    void createProgram(const char* vsSrc, const char* fsSrc,
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
            fprintf(stderr, "[GLRenderer] program link error:\n%s\n", log);
        }
        glDeleteShader(vs);
        glDeleteShader(fs);
        uProj = glGetUniformLocation(prog, "uProj");
    }

    void createQuadBuffers() {
        glGenVertexArrays(1, &m_vao);
        glBindVertexArray(m_vao);

        glGenBuffers(1, &m_vbo);
        glBindBuffer(GL_ARRAY_BUFFER, m_vbo);
        glBufferData(GL_ARRAY_BUFFER, sizeof(kQuadVerts), kQuadVerts, GL_STATIC_DRAW);
        glEnableVertexAttribArray(0);
        glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, (void*)0);

        glGenBuffers(1, &m_ibo);
        glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_ibo);
        glBufferData(GL_ELEMENT_ARRAY_BUFFER, sizeof(kQuadIndices), kQuadIndices, GL_STATIC_DRAW);

        glGenBuffers(1, &m_instVBO);
        glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
        glBufferData(GL_ARRAY_BUFFER, 0, nullptr, GL_DYNAMIC_DRAW);

        glEnableVertexAttribArray(1);
        glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, sizeof(Instance), (void*)offsetof(Instance, x));
        glVertexAttribDivisor(1, 1);

        glEnableVertexAttribArray(2);
        glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(Instance), (void*)offsetof(Instance, r));
        glVertexAttribDivisor(2, 1);

        glEnableVertexAttribArray(3);
        glVertexAttribPointer(3, 1, GL_FLOAT, GL_FALSE, sizeof(Instance), (void*)offsetof(Instance, radius));
        glVertexAttribDivisor(3, 1);

        glBindVertexArray(0);
    }

#ifdef MORPH_FEATURE_TEXT
    void createTextBuffers() {
        glGenVertexArrays(1, &m_textVAO);
        glBindVertexArray(m_textVAO);

        // Reuse same shared quad VBO + IBO
        glBindBuffer(GL_ARRAY_BUFFER, m_vbo);
        glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, (void*)0);
        glEnableVertexAttribArray(0);

        glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_ibo);

        glGenBuffers(1, &m_textInstVBO);
        glBindBuffer(GL_ARRAY_BUFFER, m_textInstVBO);
        glBufferData(GL_ARRAY_BUFFER, 0, nullptr, GL_DYNAMIC_DRAW);

        glEnableVertexAttribArray(1);
        glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, sizeof(TextInstance), (void*)offsetof(TextInstance, x));
        glVertexAttribDivisor(1, 1);

        glEnableVertexAttribArray(2);
        glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(TextInstance), (void*)offsetof(TextInstance, u1));
        glVertexAttribDivisor(2, 1);

        glEnableVertexAttribArray(3);
        glVertexAttribPointer(3, 4, GL_FLOAT, GL_FALSE, sizeof(TextInstance), (void*)offsetof(TextInstance, r));
        glVertexAttribDivisor(3, 1);

        glBindVertexArray(0);
    }

    static const char* fontPathForWeight(const std::string& weight) {
        if (weight == "bold" || weight == "700" || weight == "800" || weight == "900")
            return kDefaultFontBold;
        return kDefaultFont;
    }

    static std::string atlasKey(int fontSize, const std::string& fontWeight) {
        return fontWeight + ":" + std::to_string(fontSize);
    }

    FontAtlas& getOrCreateAtlas(int fontSize, const std::string& fontWeight = "normal") {
        std::string key = atlasKey(fontSize, fontWeight);
        auto it = m_atlases.find(key);
        if (it != m_atlases.end()) return it->second;

        FontAtlas atlas;
        atlas.fontSize = fontSize;

        const char* fontPath = fontPathForWeight(fontWeight);
        FT_Face face;
        if (FT_New_Face(m_ft, fontPath, 0, &face)) {
            fprintf(stderr, "[GLRenderer] failed to load font: %s\n", fontPath);
            return m_atlases[key] = atlas;
        }
        FT_Set_Pixel_Sizes(face, 0, fontSize);

        // Font metrics for vertical positioning
        float scale = (float)fontSize / face->units_per_EM;

        // Two-pass: first measure all advances + total pixel width
        int totalW = 0, maxH = 0;
        for (char c = 32; c < 127; c++) {
            FT_Load_Char(face, c, FT_LOAD_DEFAULT);
            int adv = (int)(face->glyph->advance.x / 64);
            // Try render to get bitmap dimensions
            int gw = 0, gh = 0;
            if (!FT_Load_Char(face, c, FT_LOAD_RENDER)) {
                gw = face->glyph->bitmap.width;
                gh = face->glyph->bitmap.rows;
            }
            totalW += (gw > 0 ? gw : adv) + 1;
            if (gh > maxH) maxH = gh;
        }
        if (totalW < 1) totalW = 1;
        if (maxH < 1) maxH = 1;

        int texW = 1; while (texW < totalW) texW <<= 1;
        int texH = 1; while (texH < maxH) texH <<= 1;
        if (texW > 4096) texW = 4096;
        if (texH > 4096) texH = 4096;

        std::vector<unsigned char> data(texW * texH, 0);
        int cx = 0;

        for (char c = 32; c < 127; c++) {
            GlyphInfo gi = {};

            // Load advance (works for all characters including space)
            FT_Load_Char(face, c, FT_LOAD_DEFAULT);
            gi.ax = (float)face->glyph->advance.x / 64.0f;
            gi.ay = (float)face->glyph->advance.y / 64.0f;

            // Try to render bitmap
            if (!FT_Load_Char(face, c, FT_LOAD_RENDER)) {
                auto& bmp = face->glyph->bitmap;

                for (int row = 0; row < bmp.rows && row < texH; row++) {
                    memcpy(data.data() + row * texW + cx,
                           bmp.buffer + row * bmp.pitch,
                           bmp.width < texW - cx ? bmp.width : texW - cx);
                }

                gi.bx = (float)face->glyph->bitmap_left;
                gi.by = (float)face->glyph->bitmap_top;
                gi.gw = (float)bmp.width;
                gi.gh = (float)bmp.rows;
                gi.u1 = (float)cx / texW;
                gi.v1 = 0.0f;
                gi.u2 = (float)(cx + bmp.width) / texW;
                gi.v2 = (float)bmp.rows / texH;

                cx += bmp.width + 1;
            } else {
                // No bitmap (space, etc.) — store advance only
                gi.gw = 0; gi.gh = 0;
                gi.u1 = gi.v1 = gi.u2 = gi.v2 = 0;
            }

            atlas.glyphs[c] = gi;
            if (cx >= texW) break;
        }

        FT_Done_Face(face);

        glGenTextures(1, &atlas.texture);
        glBindTexture(GL_TEXTURE_2D, atlas.texture);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_R8, texW, texH, 0,
                     GL_RED, GL_UNSIGNED_BYTE, data.data());
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        atlas.w = texW;
        atlas.h = texH;

        m_atlases[key] = atlas;
        return m_atlases[key];
    }
#endif

public:
    GLRenderer() = default;

    ~GLRenderer() {
        if (m_vao) glDeleteVertexArrays(1, &m_vao);
        if (m_vbo) glDeleteBuffers(1, &m_vbo);
        if (m_ibo) glDeleteBuffers(1, &m_ibo);
        if (m_instVBO) glDeleteBuffers(1, &m_instVBO);
        if (m_shader) glDeleteProgram(m_shader);
#ifdef MORPH_FEATURE_TEXT
        if (m_textVAO) glDeleteVertexArrays(1, &m_textVAO);
        if (m_textInstVBO) glDeleteBuffers(1, &m_textInstVBO);
        if (m_textShader) glDeleteProgram(m_textShader);
        for (auto& [_, a] : m_atlases)
            if (a.texture) glDeleteTextures(1, &a.texture);
        if (m_ft) FT_Done_FreeType(m_ft);
#endif
    }

    bool ensureReady() {
        if (m_ready) return true;

        createProgram(kQuadVertSrc, kQuadFragSrc, m_shader, m_uProj);
        createQuadBuffers();

#ifdef MORPH_FEATURE_TEXT
        if (FT_Init_FreeType(&m_ft)) {
            fprintf(stderr, "[GLRenderer] failed to init FreeType\n");
        }
        createProgram(kTextVertSrc, kTextFragSrc, m_textShader, m_textUProj);
        m_textUAtlas = glGetUniformLocation(m_textShader, "uAtlas");
        createTextBuffers();
#endif
        glClearColor(1,1,1,1);

        m_ready = true;
#ifdef MORPH_FEATURE_TEXT
        return m_shader != 0 && m_textShader != 0;
#else
        return m_shader != 0;
#endif
    }

    void clear() override {
        if (!ensureReady()) return;
        glEnable(GL_BLEND);
        glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    }

    void beginClip(float x, float y, float w, float h) override {
        float cx = x + m_scrollX;
        float cy = y + m_scrollY;
        glEnable(GL_SCISSOR_TEST);
        glScissor((GLint)cx, m_fbHeight - (GLint)(cy + h), (GLsizei)w, (GLsizei)h);
    }

    void endClip() override {
        glDisable(GL_SCISSOR_TEST);
    }

    void drawRect(float x, float y, float w, float h, float color[4]) override {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h, color[0], color[1], color[2], color[3], 0.0f});
    }

#ifdef MORPH_FEATURE_RADIUS
    void drawRoundedRect(float x, float y, float w, float h,
                         float radius, float color[4]) override {
        float r = radius;
        float maxR = std::min(w, h) * 0.5f;
        if (r > maxR) r = maxR;
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h, color[0], color[1], color[2], color[3], r});
    }
#endif

#ifdef MORPH_FEATURE_TEXT
    float measureTextWidth(const std::string& text,
                           float fontSize,
                           const std::string& fontWeight) override {
        if (text.empty() || fontSize < 1) return 0;
        int fs = (int)fontSize;
        auto& atlas = getOrCreateAtlas(fs, fontWeight);
        float w = 0;
        for (char c : text) {
            auto it = atlas.glyphs.find(c);
            if (it != atlas.glyphs.end()) w += it->second.ax;
        }
        return w;
    }

    void drawText(const std::string& text, float x, float y,
                  float color[4], TextAlign align,
                  float fontSize,
                  const std::string& fontWeight) override {
        if (text.empty() || fontSize < 1) return;

        int fs = (int)fontSize;
        std::string key = atlasKey(fs, fontWeight);
        auto& atlas = getOrCreateAtlas(fs, fontWeight);

        float penX = x + m_scrollX;
        // Apply scroll offset to Y; use font ascender for baseline
        float penY = y + m_scrollY + fontSize;

        // Snap to pixel grid for crisp text
        penX = std::round(penX);
        penY = std::round(penY);

        // Pre-compute width for alignment
        float totalW = 0;
        for (size_t i = 0; i < text.size(); i++) {
            auto it = atlas.glyphs.find(text[i]);
            if (it != atlas.glyphs.end()) totalW += it->second.ax;
        }

        if (align == TextAlign::Center) penX -= totalW * 0.5f;
        else if (align == TextAlign::Right) penX -= totalW;

        auto& batch = m_textBatches[key];

        for (size_t i = 0; i < text.size(); i++) {
            auto it = atlas.glyphs.find(text[i]);
            if (it == atlas.glyphs.end()) continue;
            auto& g = it->second;

            float qx = std::round(penX + g.bx);
            float qy = std::round(penY - g.by);
            float qw = g.gw;
            float qh = g.gh;

            if (qw > 0 && qh > 0) {
                batch.push_back({qx, qy, qw, qh,
                                 g.u1, g.v1, g.u2, g.v2,
                                 color[0], color[1], color[2], color[3]});
            }

            penX += g.ax;
        }
    }
#endif

    void drawTexture(unsigned int tex, float x, float y,
                     float w, float h) override {
        (void)tex; (void)x; (void)y; (void)w; (void)h;
    }

    void drawMesh(const float* verts, const unsigned int* idx,
                  int count, float color[4],
                  float x, float y, float size) override {
        (void)verts; (void)idx; (void)count; (void)color;
        (void)x; (void)y; (void)size;
    }

    void flush(const float proj[16]) {
        if (!ensureReady()) return;

        // ── Draw rects ───────────────────────────────────────
        if (!m_batch.empty()) {
            glUseProgram(m_shader);
            glUniformMatrix4fv(m_uProj, 1, GL_FALSE, proj);
            glBindVertexArray(m_vao);

            glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
            glBufferData(GL_ARRAY_BUFFER, m_batch.size() * sizeof(Instance),
                         m_batch.data(), GL_DYNAMIC_DRAW);

            glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                    (void*)0, (GLsizei)m_batch.size());
            m_batch.clear();
        }

#ifdef MORPH_FEATURE_TEXT
        // ── Draw text (per-font batch) ───────────────────────
        if (!m_textBatches.empty()) {
            glUseProgram(m_textShader);
            glUniformMatrix4fv(m_textUProj, 1, GL_FALSE, proj);
            glUniform1i(m_textUAtlas, 0);
            glBindVertexArray(m_textVAO);

            for (auto& [key, batch] : m_textBatches) {
                if (batch.empty()) continue;

                auto it = m_atlases.find(key);
                if (it == m_atlases.end() || !it->second.texture) continue;

                glActiveTexture(GL_TEXTURE0);
                glBindTexture(GL_TEXTURE_2D, it->second.texture);

                glBindBuffer(GL_ARRAY_BUFFER, m_textInstVBO);
                glBufferData(GL_ARRAY_BUFFER, batch.size() * sizeof(TextInstance),
                             batch.data(), GL_DYNAMIC_DRAW);

                glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                        (void*)0, (GLsizei)batch.size());
            }
            m_textBatches.clear();
        }
#endif

        glBindVertexArray(0);
    }
};
