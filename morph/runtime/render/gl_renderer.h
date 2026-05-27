#pragma once
#include "vendor/glad/glad.h"
#include <vector>
#include <unordered_map>
#include <cstring>
#include <cmath>
#include "../core/renderer.h"
#include "shader.h"

#ifdef MORPH_FEATURE_IMAGE
#include "../vendor/stb_image.h"
#endif

#ifdef MORPH_FEATURE_TEXT
#include <ft2build.h>
#include FT_FREETYPE_H

static const char* kDefaultFont     = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
static const char* kDefaultFontBold = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
#endif

// ── Display List Operation ───────────────────────────────────
struct DrawOp {
    enum Type : uint8_t {
        Rect,
        RoundedRect,
        BorderedRect,
        BorderedRoundedRect,
        BorderRing,
        BeginClip,
        EndClip,
        BeginRoundedClip,
        EndRoundedClip,
        PushScroll,
        PopScroll,
        Scrollbar,
        TextureQuad,
        TextureBordered,
    };
    Type type;
    float x, y, w, h;
    float r, g, b, a;           // fill color
    float data[6];               // radius, borderWidth, extra0..extra3
    float br, bg, bb, ba;       // border color
    uint32_t texId;
    void setRect(float _x, float _y, float _w, float _h, float cr[4]) {
        type = Rect; x=_x; y=_y; w=_w; h=_h;
        r=cr[0]; g=cr[1]; b=cr[2]; a=cr[3];
        for (int i=0;i<6;i++) data[i]=0;
        br=bg=bb=ba=0; texId=0;
    }
    void setRounded(float _x, float _y, float _w, float _h, float rad, float cr[4]) {
        type = RoundedRect; x=_x; y=_y; w=_w; h=_h;
        r=cr[0]; g=cr[1]; b=cr[2]; a=cr[3];
        data[0]=rad; for (int i=1;i<6;i++) data[i]=0;
        br=bg=bb=ba=0; texId=0;
    }
    void setBordered(float _x, float _y, float _w, float _h, float rad,
                     float cr[4], float bw, float bc[4]) {
        type = rad > 0 ? BorderedRoundedRect : BorderedRect;
        x=_x; y=_y; w=_w; h=_h;
        r=cr[0]; g=cr[1]; b=cr[2]; a=cr[3];
        data[0]=rad; data[1]=bw; for (int i=2;i<6;i++) data[i]=0;
        br=bc[0]; bg=bc[1]; bb=bc[2]; ba=bc[3]; texId=0;
    }
    void setClip(float _x, float _y, float _w, float _h, bool rounded, float rad) {
        type = rounded ? BeginRoundedClip : BeginClip;
        x=_x; y=_y; w=_w; h=_h;
        data[0]=rad; for (int i=1;i<6;i++) data[i]=0;
        r=g=b=a=br=bg=bb=ba=texId=0;
    }
    void setEndClip(bool rounded) {
        type = rounded ? EndRoundedClip : EndClip;
        x=y=w=h=r=g=b=a=texId=0;
        for (int i=0;i<6;i++) data[i]=0;
        br=bg=bb=ba=0;
    }
    void setScroll(float sy, bool push) {
        type = push ? PushScroll : PopScroll;
        r=sy; x=y=w=h=g=b=a=texId=0;
        for (int i=0;i<6;i++) data[i]=0;
        br=bg=bb=ba=0;
    }
};

class GLRenderer : public Renderer {
public:
    struct Instance {
        float x, y, w, h;
        float r, g, b, a;
        float radius;
        float borderWidth;
        float br, bg, bb, ba;
        float borderOnly;
    };

    std::vector<Instance> m_borderBatch;

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

#ifdef MORPH_FEATURE_IMAGE
    struct ImageInstance {
        float x, y, w, h;
        float u1, v1, u2, v2;
        float tintR, tintG, tintB, tintA;
    };
#endif

private:
    // Quad batch
    GLuint m_vao = 0, m_vbo = 0, m_ibo = 0, m_instVBO = 0;
    GLuint m_shader = 0;
    GLint m_uProj = -1;
    GLint m_uStencilMode = -1;
    int m_stencilClipDepth = 0;
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

#ifdef MORPH_FEATURE_IMAGE
    GLuint m_imageVAO = 0, m_imageVBO = 0, m_imageIBO = 0, m_imageInstVBO = 0;
    GLuint m_imageShader = 0;
    GLint m_imageUProj = -1, m_imageUTexture = -1, m_imageUStencil = -1;
    std::unordered_map<GLuint, std::vector<ImageInstance>> m_imageBatches;
    std::unordered_map<std::string, std::pair<GLuint, int>> m_textureCache; // src -> (texId, refCount)
    void createImageBuffers();
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

    void setProjection(const float proj[16]) { memcpy(m_proj, proj, sizeof(m_proj)); }

    void clear() override;
    void beginClip(float x, float y, float w, float h) override;
    void endClip() override;
    void beginRoundedClip(float x, float y, float w, float h, float radius) override;
    void endRoundedClip() override;

    void drawRect(float x, float y, float w, float h, float color[4]) override {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f});
    }

#ifdef MORPH_FEATURE_RADIUS
    void drawRoundedRect(float x, float y, float w, float h,
                         float radius, float color[4]) override {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           radius, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f});
    }
#endif

    void drawBorderedRect(float x, float y, float w, float h,
                          float color[4], float borderWidth,
                          float borderColor[4]) override {
        // Fill
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f});
        // Border ring
        m_borderBatch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                                 0.0f, 0.0f, 0.0f, 0.0f,
                                 0.0f, borderWidth,
                                 borderColor[0], borderColor[1],
                                 borderColor[2], borderColor[3],
                                 1.0f});
    }

    void drawBorderedRoundedRect(float x, float y, float w, float h,
                                 float radius, float color[4],
                                 float borderWidth,
                                 float borderColor[4]) override {
        // Fill
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           radius, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f});
        // Border ring
        m_borderBatch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                                 0.0f, 0.0f, 0.0f, 0.0f,
                                 radius, borderWidth,
                                 borderColor[0], borderColor[1],
                                 borderColor[2], borderColor[3],
                                 1.0f});
    }

    void drawBorderRing(float x, float y, float w, float h,
                        float radius, float borderWidth,
                        float borderColor[4]) override {
        m_borderBatch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                                 0.0f, 0.0f, 0.0f, 0.0f,
                                 radius, borderWidth,
                                 borderColor[0], borderColor[1],
                                 borderColor[2], borderColor[3],
                                 1.0f});
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
#ifdef MORPH_FEATURE_IMAGE
        m_imageBatches[tex].push_back({x + m_scrollX, y + m_scrollY, w, h,
                                       0.0f, 0.0f, 1.0f, 1.0f,
                                       1.0f, 1.0f, 1.0f, 1.0f});
#else
        (void)tex; (void)x; (void)y; (void)w; (void)h;
#endif
    }

    unsigned int loadTexture(const std::string& path, int& outW, int& outH) override {
#ifdef MORPH_FEATURE_IMAGE
        auto it = m_textureCache.find(path);
        if (it != m_textureCache.end()) {
            outW = it->second.second >> 16;
            outH = it->second.second & 0xFFFF;
            return it->second.first;
        }
        int n = 0;
        unsigned char* data = stbi_load(path.c_str(), &outW, &outH, &n, 4);
        if (!data) {
            fprintf(stderr, "[GLRenderer] failed to load: %s\n", path.c_str());
            return 0;
        }
        GLuint tex = 0;
        glGenTextures(1, &tex);
        glBindTexture(GL_TEXTURE_2D, tex);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, outW, outH, 0,
                     GL_RGBA, GL_UNSIGNED_BYTE, data);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        stbi_image_free(data);
        int dims = (outW << 16) | (outH & 0xFFFF);
        m_textureCache[path] = {tex, dims};
        return tex;
#else
        (void)path; (void)outW; (void)outH;
        return 0;
#endif
    }

    void drawMesh(const float* verts, const unsigned int* idx,
                  int count, float color[4],
                  float x, float y, float size) override {
        (void)verts; (void)idx; (void)count; (void)color;
        (void)x; (void)y; (void)size;
    }

    void flush(const float proj[16]);
};
