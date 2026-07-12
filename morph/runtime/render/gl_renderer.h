#pragma once
#include "vendor/glad/glad.h"
#include <vector>
#include <unordered_map>
#include <cstring>
#include <cmath>
#include "../core/renderer.h"
#include "../core/draw_op.h"
#include "shader.h"

#ifdef MORPH_FEATURE_IMAGE
#include "../vendor/stb_image.h"
#endif

#ifdef MORPH_FEATURE_TEXT
#include <ft2build.h>
#include FT_FREETYPE_H
#include <hb.h>
#include <hb-ft.h>

static const char *kDefaultFont = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
static const char *kDefaultFontBold = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
static const char *kDefaultEmojiFont = "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf";
#endif

class GLRenderer : public Renderer
{
public:
    struct Instance
    {
        float x, y, w, h;
        float r, g, b, a;
        float radius;
        float borderWidth;
        float br, bg, bb, ba;
        float borderOnly;
    };

    std::vector<Instance> m_borderBatch;

#ifdef MORPH_FEATURE_TEXT
    struct TextInstance
    {
        float x, y, w, h;
        float u1, v1, u2, v2;
        float r, g, b, a;
        float isColor; // 0.0 = R8 atlas, 1.0 = RGBA atlas
    };

    struct FontAtlas; // forward decl

    struct ShapedGlyph
    {
        unsigned int glyphIndex; // FT glyph index from HarfBuzz
        float ax, ay;            // advance
        float dx, dy;            // offset
        unsigned int codepoint;  // original Unicode codepoint
    };

    struct GlyphInfo
    {
        float ax, ay;
        float bx, by;
        float gw, gh;
        int px = 0, py = 0; // pixel position in source atlas
        bool isColor = false;
        float emojiScale = 1.0f; // scale factor for color emoji glyphs
        FontAtlas *src = nullptr; // which atlas holds the pixel data
    };

    struct FontAtlas
    {
        GLuint textureR8 = 0;
        GLuint textureRGBA = 0;
        int w = 2048, h = 2048;
        int fontSize = 0;

        // Row-packing state for R8 atlas
        int r8CursorX = 0, r8CursorY = 0, r8RowH = 0;
        // Row-packing state for RGBA atlas
        int rgbaCursorX = 0, rgbaCursorY = 0, rgbaRowH = 0;

        // Pixel buffers for incremental upload
        std::vector<unsigned char> r8Pixels;
        std::vector<unsigned char> rgbaPixels;

        // Glyphs keyed by Unicode codepoint
        std::unordered_map<unsigned int, GlyphInfo> glyphs;

        // Is this a color (RGBA) atlas vs grayscale (R8)?
        bool isColor = false;

        // Actual strike height for color emoji bitmap fonts (0 if outline font)
        int actualStrikeHeight = 0;

        // HarfBuzz + FreeType handles
        hb_font_t *hbFont = nullptr;
        FT_Face ftFace = nullptr;
    };
#endif

#ifdef MORPH_FEATURE_IMAGE
    struct ImageInstance
    {
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
    int m_scissorClipDepth = 0;
    bool m_ready = false;
    std::vector<Instance> m_batch;
    float m_proj[16] = {};

#ifdef MORPH_FEATURE_TEXT
    // Text batch — unified VAO for both R8 and RGBA glyphs
    GLuint m_textVAO = 0, m_textInstVBO = 0;
    GLuint m_textShader = 0;
    GLint m_textUProj = -1;
    GLint m_textUAtlas = -1, m_textUColorAtlas = -1;
    std::unordered_map<std::string, std::vector<TextInstance>> m_textBatches;
    FT_Library m_ft = nullptr;
    std::unordered_map<std::string, FontAtlas> m_atlases;

    // Emoji font atlas (shared across all sizes, re-created per size on demand)
    std::unordered_map<int, FontAtlas> m_emojiAtlases; // keyed by fontSize

    // UTF-8 decode helper
    static unsigned int utf8ToCodepoint(const std::string &text, size_t &pos);
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
    static const char *fontPathForWeight(const std::string &weight);
    static std::string atlasKey(int fontSize, const std::string &fontWeight);

    // Atlas management
    FontAtlas &getOrCreateAtlas(int fontSize, const std::string &fontWeight = "normal");
    FontAtlas &getOrCreateEmojiAtlas(int fontSize);
    void ensureGlyph(FontAtlas &atlas, unsigned int codepoint);
    void growAtlas(FontAtlas &atlas, bool isColor);

    // HarfBuzz shaping
    void shapeText(const std::string &text, FontAtlas &atlas,
                   std::vector<ShapedGlyph> &out);
#endif

public:
    GLRenderer() = default;
    ~GLRenderer();
    bool ensureReady();

    void setProjection(const float proj[16]) { memcpy(m_proj, proj, sizeof(m_proj)); }

    void clear() override;
    void setClearColor(float r, float g, float b, float a) override { glClearColor(r, g, b, a); }
    void beginClip(float x, float y, float w, float h) override;
    void endClip() override;
    void beginRoundedClip(float x, float y, float w, float h, float radius) override;
    void endRoundedClip() override;

    void drawRect(float x, float y, float w, float h, float color[4]) override
    {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f});
    }

#ifdef MORPH_FEATURE_RADIUS
    void drawRoundedRect(float x, float y, float w, float h,
                         float radius, float color[4]) override
    {
        m_batch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                           color[0], color[1], color[2], color[3],
                           radius, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f});
    }
#endif

    void drawBorderedRect(float x, float y, float w, float h,
                          float color[4], float borderWidth,
                          float borderColor[4]) override
    {
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
                                 float borderColor[4]) override
    {
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
                        float borderColor[4]) override
    {
        m_borderBatch.push_back({x + m_scrollX, y + m_scrollY, w, h,
                                 0.0f, 0.0f, 0.0f, 0.0f,
                                 radius, borderWidth,
                                 borderColor[0], borderColor[1],
                                 borderColor[2], borderColor[3],
                                 1.0f});
    }

#ifdef MORPH_FEATURE_TEXT
    float measureTextWidth(const std::string &text, float fontSize,
                           const std::string &fontWeight) override;
    void drawText(const std::string &text, float x, float y,
                  float color[4], TextAlign align,
                  float fontSize,
                  const std::string &fontWeight) override;
#endif

    void drawTexture(unsigned int tex, float x, float y, float w, float h) override
    {
#ifdef MORPH_FEATURE_IMAGE
        m_imageBatches[tex].push_back({x + m_scrollX, y + m_scrollY, w, h,
                                       0.0f, 0.0f, 1.0f, 1.0f,
                                       1.0f, 1.0f, 1.0f, 1.0f});
#else
        (void)tex;
        (void)x;
        (void)y;
        (void)w;
        (void)h;
#endif
    }

    unsigned int loadTexture(const std::string &path, int &outW, int &outH) override
    {
#ifdef MORPH_FEATURE_IMAGE
        auto it = m_textureCache.find(path);
        if (it != m_textureCache.end())
        {
            outW = it->second.second >> 16;
            outH = it->second.second & 0xFFFF;
            return it->second.first;
        }
        int n = 0;
        unsigned char *data = stbi_load(path.c_str(), &outW, &outH, &n, 4);
        if (!data)
        {
            fprintf(stderr, "[GLRenderer] failed to load: %s\n", path.c_str());
            return 0;
        }
        GLuint tex = 0;
        glGenTextures(1, &tex);
        glBindTexture(GL_TEXTURE_2D, tex);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, outW, outH, 0,
                     GL_RGBA, GL_UNSIGNED_BYTE, data);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
        glGenerateMipmap(GL_TEXTURE_2D);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
        glTexParameterf(GL_TEXTURE_2D, GL_TEXTURE_LOD_BIAS, -1.0f);

        stbi_image_free(data);
        int dims = (outW << 16) | (outH & 0xFFFF);
        m_textureCache[path] = {tex, dims};
        return tex;
#else
        (void)path;
        (void)outW;
        (void)outH;
        return 0;
#endif
    }

    void drawMesh(const float *verts, const unsigned int *idx,
                  int count, float color[4],
                  float x, float y, float size) override
    {
        (void)verts;
        (void)idx;
        (void)count;
        (void)color;
        (void)x;
        (void)y;
        (void)size;
    }

    void flush(const float proj[16]);
};
