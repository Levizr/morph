#pragma once
#include "vendor/glad/glad.h"
#include <vector>
#include <unordered_map>
#include <cstring>
#include <cmath>
#include "../core/renderer.h"
#include "shader.h"

#ifdef MORPH_FEATURE_TEXT
#include <ft2build.h>
#include FT_FREETYPE_H

static const char* kDefaultFont     = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
static const char* kDefaultFontBold = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
#endif

class GLRenderer : public Renderer {
public:
    struct Instance {
        float x, y, w, h;
        float r, g, b, a;
        float radius;
        float borderWidth;
        float br, bg, bb, ba;
    };

#ifdef MORPH_FEATURE_TEXT
    struct TextInstance {
        float x, y, w, h;
        float u1, v1, u2, v2;
        float r, g, b, a;
    };

    struct GlyphInfo {
        float ax, ay;
        float bx, by;
        float gw, gh;
        float u1, v1, u2, v2;
    };

    struct FontAtlas {
        GLuint texture = 0;
        int w = 0, h = 0;
        int fontSize = 0;
        std::unordered_map<GLchar, GlyphInfo> glyphs;
    };
#endif

private:
    // Quad batch
    GLuint m_vao = 0, m_vbo = 0, m_ibo = 0, m_instVBO = 0;
    GLuint m_shader = 0;
    GLint m_uProj = -1;
    bool m_ready = false;
    std::vector<Instance> m_batch;
    float m_proj[16] = {};

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

    void createQuadBuffers();
#ifdef MORPH_FEATURE_TEXT
    void createTextBuffers();
    static const char* fontPathForWeight(const std::string& weight);
    static std::string atlasKey(int fontSize, const std::string& fontWeight);
    FontAtlas& getOrCreateAtlas(int fontSize, const std::string& fontWeight = "normal");
#endif

public:
    GLRenderer() = default;
    ~GLRenderer();
    bool ensureReady();

    void clear() override;
    void beginClip(float x, float y, float w, float h) override;
    void endClip() override;

    void drawRect(float x, float y, float w, float h, float color[4]) override {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f});
    }

#ifdef MORPH_FEATURE_RADIUS
    void drawRoundedRect(float x, float y, float w, float h,
                         float radius, float color[4]) override {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           radius, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f});
    }
#endif

    void drawBorderedRect(float x, float y, float w, float h,
                          float color[4], float borderWidth,
                          float borderColor[4]) override {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           0.0f, borderWidth,
                           borderColor[0], borderColor[1],
                           borderColor[2], borderColor[3]});
    }

    void drawBorderedRoundedRect(float x, float y, float w, float h,
                                 float radius, float color[4],
                                 float borderWidth,
                                 float borderColor[4]) override {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           radius, borderWidth,
                           borderColor[0], borderColor[1],
                           borderColor[2], borderColor[3]});
    }

#ifdef MORPH_FEATURE_TEXT
    float measureTextWidth(const std::string& text, float fontSize,
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
        auto& atlas = getOrCreateAtlas(fs, fontWeight);

        float penX = std::round(x + m_scrollX);
        float penY = std::round(y + m_scrollY + fontSize);

        float totalW = 0;
        for (size_t i = 0; i < text.size(); i++) {
            auto it = atlas.glyphs.find(text[i]);
            if (it != atlas.glyphs.end()) totalW += it->second.ax;
        }
        if (align == TextAlign::Center) penX -= totalW * 0.5f;
        else if (align == TextAlign::Right) penX -= totalW;

        auto& batch = m_textBatches[atlasKey(fs, fontWeight)];
        for (size_t i = 0; i < text.size(); i++) {
            auto it = atlas.glyphs.find(text[i]);
            if (it == atlas.glyphs.end()) continue;
            auto& g = it->second;
            float qx = std::round(penX + g.bx);
            float qy = std::round(penY - g.by);
            if (g.gw > 0 && g.gh > 0) {
                batch.push_back({qx, qy, g.gw, g.gh, g.u1, g.v1, g.u2, g.v2,
                                 color[0], color[1], color[2], color[3]});
            }
            penX += g.ax;
        }
    }
#endif

    void drawTexture(unsigned int tex, float x, float y, float w, float h) override {
        (void)tex; (void)x; (void)y; (void)w; (void)h;
    }

    void drawMesh(const float* verts, const unsigned int* idx,
                  int count, float color[4],
                  float x, float y, float size) override {
        (void)verts; (void)idx; (void)count; (void)color;
        (void)x; (void)y; (void)size;
    }

    void flush(const float proj[16]);
};
