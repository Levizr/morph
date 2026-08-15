#include "gl_renderer.h"
#include <vector>

#ifdef MORPH_FEATURE_IMAGE
void GLRenderer::createImageBuffers()
{
    glGenVertexArrays(1, &m_imageVAO);
    glBindVertexArray(m_imageVAO);

    glBindBuffer(GL_ARRAY_BUFFER, m_vbo);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, (void *)0);
    glEnableVertexAttribArray(0);

    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_ibo);

    glGenBuffers(1, &m_imageInstVBO);
    glBindBuffer(GL_ARRAY_BUFFER, m_imageInstVBO);
    glBufferData(GL_ARRAY_BUFFER, 0, nullptr, GL_DYNAMIC_DRAW);

    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, sizeof(ImageInstance), (void *)offsetof(ImageInstance, x));
    glVertexAttribDivisor(1, 1);

    glEnableVertexAttribArray(2);
    glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(ImageInstance), (void *)offsetof(ImageInstance, u1));
    glVertexAttribDivisor(2, 1);

    glEnableVertexAttribArray(3);
    glVertexAttribPointer(3, 4, GL_FLOAT, GL_FALSE, sizeof(ImageInstance), (void *)offsetof(ImageInstance, tintR));
    glVertexAttribDivisor(3, 1);

    glBindVertexArray(0);
}
#endif

void GLRenderer::createQuadBuffers()
{
    glGenVertexArrays(1, &m_vao);
    glBindVertexArray(m_vao);

    glGenBuffers(1, &m_vbo);
    glBindBuffer(GL_ARRAY_BUFFER, m_vbo);
    glBufferData(GL_ARRAY_BUFFER, sizeof(kQuadVerts), kQuadVerts, GL_STATIC_DRAW);
    glEnableVertexAttribArray(0);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, (void *)0);

    glGenBuffers(1, &m_ibo);
    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_ibo);
    glBufferData(GL_ELEMENT_ARRAY_BUFFER, sizeof(kQuadIndices), kQuadIndices, GL_STATIC_DRAW);

    glGenBuffers(1, &m_instVBO);
    glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
    glBufferData(GL_ARRAY_BUFFER, 0, nullptr, GL_DYNAMIC_DRAW);

    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, sizeof(Instance), (void *)offsetof(Instance, x));
    glVertexAttribDivisor(1, 1);

    glEnableVertexAttribArray(2);
    glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(Instance), (void *)offsetof(Instance, r));
    glVertexAttribDivisor(2, 1);

    glEnableVertexAttribArray(3);
    glVertexAttribPointer(3, 1, GL_FLOAT, GL_FALSE, sizeof(Instance), (void *)offsetof(Instance, radius));
    glVertexAttribDivisor(3, 1);

    glEnableVertexAttribArray(4);
    glVertexAttribPointer(4, 1, GL_FLOAT, GL_FALSE, sizeof(Instance), (void *)offsetof(Instance, borderWidth));
    glVertexAttribDivisor(4, 1);

    glEnableVertexAttribArray(5);
    glVertexAttribPointer(5, 4, GL_FLOAT, GL_FALSE, sizeof(Instance), (void *)offsetof(Instance, br));
    glVertexAttribDivisor(5, 1);

    glEnableVertexAttribArray(6);
    glVertexAttribPointer(6, 1, GL_FLOAT, GL_FALSE, sizeof(Instance), (void *)offsetof(Instance, borderOnly));
    glVertexAttribDivisor(6, 1);

    glBindVertexArray(0);
}

#ifdef MORPH_FEATURE_TEXT
void GLRenderer::createTextBuffers()
{
    glGenVertexArrays(1, &m_textVAO);
    glBindVertexArray(m_textVAO);

    glBindBuffer(GL_ARRAY_BUFFER, m_vbo);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, (void *)0);
    glEnableVertexAttribArray(0);

    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_ibo);

    glGenBuffers(1, &m_textInstVBO);
    glBindBuffer(GL_ARRAY_BUFFER, m_textInstVBO);
    glBufferData(GL_ARRAY_BUFFER, 0, nullptr, GL_DYNAMIC_DRAW);

    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, sizeof(TextInstance), (void *)offsetof(TextInstance, x));
    glVertexAttribDivisor(1, 1);

    glEnableVertexAttribArray(2);
    glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(TextInstance), (void *)offsetof(TextInstance, u1));
    glVertexAttribDivisor(2, 1);

    glEnableVertexAttribArray(3);
    glVertexAttribPointer(3, 4, GL_FLOAT, GL_FALSE, sizeof(TextInstance), (void *)offsetof(TextInstance, r));
    glVertexAttribDivisor(3, 1);

    glEnableVertexAttribArray(4);
    glVertexAttribPointer(4, 1, GL_FLOAT, GL_FALSE, sizeof(TextInstance), (void *)offsetof(TextInstance, isColor));
    glVertexAttribDivisor(4, 1);

    glBindVertexArray(0);
}

const std::string &GLRenderer::fontPathForWeight(const std::string &weight)
{
    if (m_fontPath.empty())
        m_fontPath = morphResolveFont(kRegularFontCandidates);
    if (m_fontPathBold.empty())
        m_fontPathBold = morphResolveFont(kBoldFontCandidates);
    if (weight == "bold" || weight == "700" || weight == "800" || weight == "900")
        return m_fontPathBold;
    return m_fontPath;
}

std::string GLRenderer::atlasKey(int fontSize, const std::string &fontWeight)
{
    return fontWeight + ":" + std::to_string(fontSize);
}

unsigned int GLRenderer::utf8ToCodepoint(const std::string &text, size_t &pos)
{
    if (pos >= text.size())
        return 0;
    unsigned char lead = (unsigned char)text[pos];
    if (lead < 0x80)
    {
        unsigned int cp = lead;
        pos += 1;
        return cp;
    }
    if ((lead & 0xE0) == 0xC0 && pos + 1 < text.size())
    {
        unsigned int cp = ((lead & 0x1F) << 6) | ((unsigned char)text[pos + 1] & 0x3F);
        pos += 2;
        return cp;
    }
    if ((lead & 0xF0) == 0xE0 && pos + 2 < text.size())
    {
        unsigned int cp = ((lead & 0x0F) << 12) | (((unsigned char)text[pos + 1] & 0x3F) << 6) | ((unsigned char)text[pos + 2] & 0x3F);
        pos += 3;
        return cp;
    }
    if ((lead & 0xF8) == 0xF0 && pos + 3 < text.size())
    {
        unsigned int cp = ((lead & 0x07) << 18) | (((unsigned char)text[pos + 1] & 0x3F) << 12) | (((unsigned char)text[pos + 2] & 0x3F) << 6) | ((unsigned char)text[pos + 3] & 0x3F);
        pos += 4;
        return cp;
    }
    pos += 1;
    return 0;
}

GLRenderer::FontAtlas &GLRenderer::getOrCreateAtlas(int fontSize, const std::string &fontWeight)
{
    std::string key = atlasKey(fontSize, fontWeight);
    auto it = m_atlases.find(key);
    if (it != m_atlases.end())
        return it->second;

    FontAtlas atlas;
    atlas.fontSize = fontSize;
    atlas.w = 256;
    atlas.h = 256;

    const char *fontPath = fontPathForWeight(fontWeight).c_str();
    FT_Face face;
    if (FT_New_Face(m_ft, fontPath, 0, &face))
    {
        fprintf(stderr, "[GLRenderer] failed to load font: %s\n", fontPath);
        return m_atlases[key] = atlas;
    }
    FT_Set_Pixel_Sizes(face, 0, fontSize);

    // Create HarfBuzz font
    hb_font_t *hbFont = hb_ft_font_create(face, nullptr);
    hb_ft_font_changed(hbFont);
    atlas.hbFont = hbFont;
    atlas.ftFace = face;

    // Create R8 texture (initially all zeros, grows on demand)
    glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
    glGenTextures(1, &atlas.textureR8);
    glBindTexture(GL_TEXTURE_2D, atlas.textureR8);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_R8, atlas.w, atlas.h, 0, GL_RED, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glGenerateMipmap(GL_TEXTURE_2D);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR_MIPMAP_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

    m_atlases[key] = atlas;
    return m_atlases[key];
}

GLRenderer::FontAtlas &GLRenderer::getOrCreateEmojiAtlas(int fontSize)
{
    auto it = m_emojiAtlases.find(fontSize);
    if (it != m_emojiAtlases.end())
        return it->second;

    FontAtlas atlas;
    atlas.fontSize = fontSize;
    atlas.w = 256;
    atlas.h = 256;
    atlas.isColor = true;

    if (m_emojiFontPath.empty())
        m_emojiFontPath = morphResolveFont(kEmojiFontCandidates);
    FT_Face face;
    if (m_emojiFontPath.empty() || FT_New_Face(m_ft, m_emojiFontPath.c_str(), 0, &face))
    {
        fprintf(stderr, "[GLRenderer] failed to load emoji font\n");
        return m_emojiAtlases[fontSize] = atlas;
    }
    if (FT_HAS_COLOR(face) && face->num_fixed_sizes > 0)
    {
        int best = 0;
        for (int i = 1; i < face->num_fixed_sizes; i++)
        {
            if (abs((int)face->available_sizes[i].height - fontSize) <
                abs((int)face->available_sizes[best].height - fontSize))
                best = i;
        }
        FT_Select_Size(face, best);
        atlas.actualStrikeHeight = face->available_sizes[best].height;
    }
    else
    {
        FT_Set_Pixel_Sizes(face, 0, fontSize);
        atlas.actualStrikeHeight = fontSize;
    }
    atlas.ftFace = face;

    // Create RGBA texture (initially empty, grows on demand)
    glGenTextures(1, &atlas.textureRGBA);
    glBindTexture(GL_TEXTURE_2D, atlas.textureRGBA);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, atlas.w, atlas.h, 0, GL_BGRA, GL_UNSIGNED_BYTE, nullptr);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);

    m_emojiAtlases[fontSize] = atlas;
    return m_emojiAtlases[fontSize];
}

void GLRenderer::growAtlas(FontAtlas &atlas, bool isColor)
{
    int oldW = atlas.w;
    int oldH = atlas.h;
    int newW = oldW;
    int newH = oldH * 2;
    if (newH > 4096)
    {
        newW = oldW * 2;
        newH = oldH;
    }
    if (newW > 4096)
        return; // max size

    if (isColor)
    {
        std::vector<unsigned char> newPixels(newW * newH * 4, 0);
        for (int y = 0; y < oldH; y++)
            memcpy(newPixels.data() + y * newW * 4, atlas.rgbaPixels.data() + y * oldW * 4, oldW * 4);
        atlas.rgbaPixels = std::move(newPixels);
        atlas.w = newW;
        atlas.h = newH;
        glBindTexture(GL_TEXTURE_2D, atlas.textureRGBA);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, newW, newH, 0, GL_BGRA, GL_UNSIGNED_BYTE, atlas.rgbaPixels.data());
    }
    else
    {
        std::vector<unsigned char> newPixels(newW * newH, 0);
        for (int y = 0; y < oldH; y++)
            memcpy(newPixels.data() + y * newW, atlas.r8Pixels.data() + y * oldW, oldW);
        atlas.r8Pixels = std::move(newPixels);
        atlas.w = newW;
        atlas.h = newH;
        glBindTexture(GL_TEXTURE_2D, atlas.textureR8);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_R8, newW, newH, 0, GL_RED, GL_UNSIGNED_BYTE, atlas.r8Pixels.data());
    }
}

static void ensureAtlasBuffer(GLRenderer::FontAtlas &atlas, bool isColor)
{
    if (isColor)
    {
        if (atlas.rgbaPixels.empty())
            atlas.rgbaPixels.resize(atlas.w * atlas.h * 4, 0);
    }
    else
    {
        if (atlas.r8Pixels.empty())
            atlas.r8Pixels.resize(atlas.w * atlas.h, 0);
    }
}

void GLRenderer::ensureGlyph(FontAtlas &atlas, unsigned int codepoint)
{
    if (atlas.glyphs.count(codepoint))
        return;

    FT_Face face = atlas.ftFace;
    if (!face)
        return;

    // Try loading the glyph
    FT_UInt glyphIndex = FT_Get_Char_Index(face, codepoint);
    FontAtlas *renderAtlas = &atlas;
    bool isColor = false;

    // Check emoji font — prefer it for non-ASCII codepoints when available
    auto emojiIt = m_emojiAtlases.find(atlas.fontSize);
    if (emojiIt != m_emojiAtlases.end() && emojiIt->second.ftFace)
    {
        FT_UInt emojiGlyph = FT_Get_Char_Index(emojiIt->second.ftFace, codepoint);
        if (emojiGlyph != 0 && (glyphIndex == 0 || codepoint >= 0x80))
        {
            renderAtlas = &emojiIt->second;
            face = renderAtlas->ftFace;
            glyphIndex = emojiGlyph;
            isColor = true;
        }
    }

    // Load and render the glyph
    FT_Int32 loadFlags = FT_LOAD_RENDER;
    if (isColor)
        loadFlags |= FT_LOAD_COLOR;
    if (FT_Load_Glyph(face, glyphIndex, loadFlags))
    {
        // Store empty glyph to avoid retrying
        GlyphInfo gi = {};
        gi.isColor = isColor;
        gi.src = renderAtlas;
        atlas.glyphs[codepoint] = gi;
        return;
    }

    auto &bmp = face->glyph->bitmap;
    // Determine color from actual pixel mode, not from font selection
    bool isColorGlyph = (bmp.pixel_mode == FT_PIXEL_MODE_BGRA);
    GlyphInfo gi;
    gi.ax = (float)face->glyph->advance.x / 64.0f;
    gi.ay = (float)face->glyph->advance.y / 64.0f;
    gi.bx = (float)face->glyph->bitmap_left;
    gi.by = (float)face->glyph->bitmap_top;
    gi.gw = (float)bmp.width;
    gi.gh = (float)bmp.rows;
    gi.isColor = isColorGlyph;
    gi.src = renderAtlas;
    if (isColorGlyph && renderAtlas->actualStrikeHeight > 0)
        gi.emojiScale = (float)atlas.fontSize / (float)renderAtlas->actualStrikeHeight;

    ensureAtlasBuffer(*renderAtlas, isColorGlyph);

    int glyphW = bmp.width > 0 ? (int)bmp.width : 1;
    int glyphH = bmp.rows > 0 ? (int)bmp.rows : 1;
    int pad = 1;

    // Row packing
    int &cursorX = isColorGlyph ? renderAtlas->rgbaCursorX : renderAtlas->r8CursorX;
    int &cursorY = isColorGlyph ? renderAtlas->rgbaCursorY : renderAtlas->r8CursorY;
    int &rowH = isColorGlyph ? renderAtlas->rgbaRowH : renderAtlas->r8RowH;

    if (cursorX + glyphW + pad > renderAtlas->w)
    {
        cursorX = 0;
        cursorY += rowH + pad;
        rowH = 0;
    }
    while (cursorY + glyphH + pad > renderAtlas->h)
    {
        growAtlas(*renderAtlas, isColor);
    }
    if (glyphH > rowH)
        rowH = glyphH;

    // Store pixel coordinates before blit (atlas dimensions are final after grow)
    gi.px = cursorX;
    gi.py = cursorY;

    // Blit glyph into pixel buffer
    if (bmp.width > 0 && bmp.rows > 0)
    {
        glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
        if (isColorGlyph)
        {
            for (int row = 0; row < (int)bmp.rows; row++)
            {
                memcpy(renderAtlas->rgbaPixels.data() + (cursorY + row) * renderAtlas->w * 4 + cursorX * 4,
                       bmp.buffer + row * bmp.pitch,
                       bmp.width * 4);
            }
            glBindTexture(GL_TEXTURE_2D, renderAtlas->textureRGBA);
            glPixelStorei(GL_UNPACK_ROW_LENGTH, renderAtlas->w);
            glTexSubImage2D(GL_TEXTURE_2D, 0, cursorX, cursorY, glyphW, glyphH,
                            GL_BGRA, GL_UNSIGNED_BYTE,
                            renderAtlas->rgbaPixels.data() + cursorY * renderAtlas->w * 4 + cursorX * 4);
            glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);
        }
        else
        {
            for (int row = 0; row < (int)bmp.rows; row++)
            {
                memcpy(renderAtlas->r8Pixels.data() + (cursorY + row) * renderAtlas->w + cursorX,
                       bmp.buffer + row * bmp.pitch,
                       bmp.width);
            }
            glBindTexture(GL_TEXTURE_2D, renderAtlas->textureR8);
            glPixelStorei(GL_UNPACK_ROW_LENGTH, renderAtlas->w);
            glTexSubImage2D(GL_TEXTURE_2D, 0, cursorX, cursorY, glyphW, glyphH,
                            GL_RED, GL_UNSIGNED_BYTE,
                            renderAtlas->r8Pixels.data() + cursorY * renderAtlas->w + cursorX);
            glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);
        }
        glPixelStorei(GL_UNPACK_ALIGNMENT, 4);
    }

    cursorX += glyphW + pad;

    // Store glyph — always in the REQUESTING atlas, with src pointing to the
    // atlas that actually has the pixel data (same atlas or emoji atlas)
    atlas.glyphs[codepoint] = gi;
    // Also store in the render atlas if different (so emoji atlas also has it)
    if (renderAtlas != &atlas)
    {
        renderAtlas->glyphs[codepoint] = gi;
    }
}

void GLRenderer::shapeText(const std::string &text, FontAtlas &atlas,
                           std::vector<ShapedGlyph> &out)
{
    out.clear();
    if (text.empty() || !atlas.hbFont)
        return;

    hb_buffer_t *buf = hb_buffer_create();
    hb_buffer_add_utf8(buf, text.c_str(), (int)text.size(), 0, (int)text.size());
    hb_buffer_set_direction(buf, HB_DIRECTION_LTR);
    hb_buffer_set_script(buf, HB_SCRIPT_COMMON);
    hb_buffer_set_language(buf, hb_language_from_string("en", -1));
    hb_shape(atlas.hbFont, buf, nullptr, 0);

    unsigned int glyphCount = 0;
    hb_glyph_info_t *glyphInfo = hb_buffer_get_glyph_infos(buf, &glyphCount);
    hb_glyph_position_t *glyphPos = hb_buffer_get_glyph_positions(buf, &glyphCount);

    for (unsigned int i = 0; i < glyphCount; i++)
    {
        ShapedGlyph sg;
        sg.glyphIndex = glyphInfo[i].codepoint;
        sg.ax = glyphPos[i].x_advance / 64.0f;
        sg.ay = glyphPos[i].y_advance / 64.0f;
        sg.dx = glyphPos[i].x_offset / 64.0f;
        sg.dy = glyphPos[i].y_offset / 64.0f;

        // Extract original codepoint from cluster byte offset
        size_t clusterPos = glyphInfo[i].cluster;
        sg.codepoint = utf8ToCodepoint(text, clusterPos);

        out.push_back(sg);
    }

    hb_buffer_destroy(buf);
}

float GLRenderer::measureTextWidth(const std::string &text, float fontSize,
                                   const std::string &fontWeight)
{
    if (text.empty() || fontSize < 1)
        return 0;
    int fs = (int)fontSize;
    auto &atlas = getOrCreateAtlas(fs, fontWeight);

    std::vector<ShapedGlyph> shaped;
    shapeText(text, atlas, shaped);

    float w = 0;
    for (auto &sg : shaped)
        w += sg.ax;
    return w;
}

void GLRenderer::drawText(const std::string &text, float x, float y,
                          float color[4], TextAlign align,
                          float fontSize,
                          const std::string &fontWeight)
{
    if (text.empty() || fontSize < 1)
        return;

    int fs = (int)fontSize;
    auto &atlas = getOrCreateAtlas(fs, fontWeight);

    // Ensure emoji atlas exists if emoji font is loadable
    FontAtlas *emojiAtlas = nullptr;
    {
        auto &ea = getOrCreateEmojiAtlas(fs);
        if (ea.ftFace)
            emojiAtlas = &ea;
    }

    // Shape the text
    std::vector<ShapedGlyph> shaped;
    shapeText(text, atlas, shaped);

    if (shaped.empty())
        return;

    float penX = std::round(x + m_scrollX);

    // Measure the run's glyph ink bounds (relative to the baseline) so the
    // text is optically centered inside its line box (height = 1.4em).
    // Placing the baseline at y + fontSize only centers the em box, which
    // leaves the visible ink a few px above the middle.
    float inkTop = 0.0f, inkBottom = 0.0f;
    bool haveInk = false;
    for (auto &sg : shaped)
    {
        ensureGlyph(atlas, sg.codepoint);
        auto it = atlas.glyphs.find(sg.codepoint);
        if (it == atlas.glyphs.end())
            continue;
        auto &g = it->second;
        if (g.gw <= 0.0f || g.gh <= 0.0f)
            continue;
        float es = g.emojiScale;
        float top = -g.by * es + sg.dy;
        float bottom = top + g.gh * es;
        if (!haveInk)
        {
            inkTop = top;
            inkBottom = bottom;
            haveInk = true;
        }
        else
        {
            if (top < inkTop) inkTop = top;
            if (bottom > inkBottom) inkBottom = bottom;
        }
    }

    float penY = std::round(y + m_scrollY + fontSize);
    if (haveInk)
        penY = std::round(y + m_scrollY + fontSize * 1.4f * 0.5f
                          - (inkTop + inkBottom) * 0.5f);

    // Measure total width for alignment
    float totalW = 0;
    for (auto &sg : shaped)
        totalW += sg.ax;

    if (align == TextAlign::Center)
        penX -= totalW * 0.5f;
    else if (align == TextAlign::Right)
        penX -= totalW;

    auto &batch = m_textBatches[atlasKey(fs, fontWeight)];

    for (auto &sg : shaped)
    {
        // Ensure glyph is rendered (may route to emoji atlas)
        ensureGlyph(atlas, sg.codepoint);

        auto it = atlas.glyphs.find(sg.codepoint);
        if (it == atlas.glyphs.end())
        {
            penX += sg.ax;
            continue;
        }
        auto &g = it->second;

        float es = g.emojiScale;
        float qx = std::round(penX + g.bx * es + sg.dx);
        float qy = std::round(penY - g.by * es + sg.dy);

        if (g.gw > 0 && g.gh > 0)
        {
            // Compute UVs from pixel coords + source atlas dimensions
            FontAtlas *src = g.src ? g.src : &atlas;
            float u1 = ((float)g.px + 0.5f) / src->w;
            float v1 = ((float)g.py + 0.5f) / src->h;
            float u2 = ((float)(g.px + (int)g.gw) - 0.5f) / src->w;
            float v2 = ((float)(g.py + (int)g.gh) - 0.5f) / src->h;
            batch.push_back({qx, qy, g.gw * es, g.gh * es,
                             u1, v1, u2, v2,
                             color[0], color[1], color[2], color[3],
                             g.isColor ? 1.0f : 0.0f});
        }
        penX += es != 1.0f ? g.ax * es : sg.ax;
    }
}
#endif

GLRenderer::~GLRenderer()
{
    if (m_vao)
        glDeleteVertexArrays(1, &m_vao);
    if (m_vbo)
        glDeleteBuffers(1, &m_vbo);
    if (m_ibo)
        glDeleteBuffers(1, &m_ibo);
    if (m_instVBO)
        glDeleteBuffers(1, &m_instVBO);
    if (m_shader)
        glDeleteProgram(m_shader);
#ifdef MORPH_FEATURE_TEXT
    if (m_textVAO)
        glDeleteVertexArrays(1, &m_textVAO);
    if (m_textInstVBO)
        glDeleteBuffers(1, &m_textInstVBO);
    if (m_textShader)
        glDeleteProgram(m_textShader);
    for (auto &[_, a] : m_atlases)
    {
        if (a.textureR8)
            glDeleteTextures(1, &a.textureR8);
        if (a.textureRGBA)
            glDeleteTextures(1, &a.textureRGBA);
        if (a.hbFont)
            hb_font_destroy(a.hbFont);
        if (a.ftFace)
            FT_Done_Face(a.ftFace);
    }
    for (auto &[_, a] : m_emojiAtlases)
    {
        if (a.textureRGBA)
            glDeleteTextures(1, &a.textureRGBA);
        if (a.ftFace)
            FT_Done_Face(a.ftFace);
    }
    if (m_ft)
        FT_Done_FreeType(m_ft);
#endif
#ifdef MORPH_FEATURE_IMAGE
    if (m_imageVAO)
        glDeleteVertexArrays(1, &m_imageVAO);
    if (m_imageInstVBO)
        glDeleteBuffers(1, &m_imageInstVBO);
    if (m_imageShader)
        glDeleteProgram(m_imageShader);
    for (auto &[_, pair] : m_textureCache)
        if (pair.first)
            glDeleteTextures(1, &pair.first);
#endif
}

bool GLRenderer::ensureReady()
{
    if (m_ready)
        return true;

    createProgram(kQuadVertSrc, kQuadFragSrc, m_shader, m_uProj);
    m_uStencilMode = glGetUniformLocation(m_shader, "uStencilMode");
    createQuadBuffers();

#ifdef MORPH_FEATURE_TEXT
    if (FT_Init_FreeType(&m_ft))
    {
        fprintf(stderr, "[GLRenderer] failed to init FreeType\n");
    }
    createProgram(kTextVertSrc, kTextFragSrc, m_textShader, m_textUProj);
    m_textUAtlas = glGetUniformLocation(m_textShader, "uAtlas");
    m_textUColorAtlas = glGetUniformLocation(m_textShader, "uColorAtlas");
    createTextBuffers();
#endif

#ifdef MORPH_FEATURE_IMAGE
    createProgram(kImageVertSrc, kImageFragSrc, m_imageShader, m_imageUProj);
    m_imageUTexture = glGetUniformLocation(m_imageShader, "uTexture");
    m_imageUStencil = glGetUniformLocation(m_imageShader, "uStencilMode");
    createImageBuffers();
#endif

    m_ready = true;
#if defined(MORPH_FEATURE_TEXT) && defined(MORPH_FEATURE_IMAGE)
    return m_shader != 0 && m_textShader != 0 && m_imageShader != 0;
#elif defined(MORPH_FEATURE_TEXT)
    return m_shader != 0 && m_textShader != 0;
#elif defined(MORPH_FEATURE_IMAGE)
    return m_shader != 0 && m_imageShader != 0;
#else
    return m_shader != 0;
#endif
}

void GLRenderer::clear()
{
    if (!ensureReady())
        return;
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
    m_stencilClipDepth = 0;
}

void GLRenderer::beginClip(float x, float y, float w, float h)
{
    float cx = x + m_scrollX;
    float cy = y + m_scrollY;
    glEnable(GL_SCISSOR_TEST);
    glScissor((GLint)cx, m_fbHeight - (GLint)(cy + h), (GLsizei)w, (GLsizei)h);
    m_scissorClipDepth++;
}

void GLRenderer::endClip()
{
    m_scissorClipDepth--;
    if (m_scissorClipDepth <= 0)
    {
        m_scissorClipDepth = 0;
        glDisable(GL_SCISSOR_TEST);
    }
}

void GLRenderer::beginRoundedClip(float x, float y, float w, float h, float radius)
{
    flush(m_proj);

    // Use GL_INCR so nested masks properly intersect:
    //   Level 1: stencil goes 0→1 for fragments inside shape 1.
    //   Level 2: stencil goes 1→2 for fragments inside BOTH shape 1 AND shape 2.
    // Because discard in the shader prevents INCR for fragments outside the shape,
    // only the intersection of all ancestor masks plus this shape passes.
    glEnable(GL_STENCIL_TEST);
    glStencilFunc(GL_EQUAL, m_stencilClipDepth, 0xFF);
    glStencilOp(GL_KEEP, GL_KEEP, GL_INCR);
    glColorMask(GL_FALSE, GL_FALSE, GL_FALSE, GL_FALSE);
    glDepthMask(GL_FALSE);

    glUseProgram(m_shader);
    glUniformMatrix4fv(m_uProj, 1, GL_FALSE, m_proj);
    glUniform1i(m_uStencilMode, 1);

    Instance inst = {x + m_scrollX, y + m_scrollY,
                     w, h,
                     1.0f, 1.0f, 1.0f, 1.0f,
                     radius, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
    glBindVertexArray(m_vao);
    glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
    glBufferData(GL_ARRAY_BUFFER, sizeof(Instance), &inst, GL_DYNAMIC_DRAW);
    glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT, (void *)0, 1);

    glUniform1i(m_uStencilMode, 0);
    glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);
    glDepthMask(GL_TRUE);

    int newRef = m_stencilClipDepth + 1;
    glStencilFunc(GL_EQUAL, newRef, 0xFF);
    glStencilOp(GL_KEEP, GL_KEEP, GL_KEEP);

    m_stencilClipDepth++;
}

void GLRenderer::endRoundedClip()
{
    flush(m_proj);
    m_stencilClipDepth--;
    if (m_stencilClipDepth > 0)
    {
        glStencilFunc(GL_EQUAL, m_stencilClipDepth, 0xFF);
    }
    else
    {
        glDisable(GL_STENCIL_TEST);
        m_stencilClipDepth = 0;
    }
}

void GLRenderer::flush(const float proj[16])
{
    if (!ensureReady())
        return;

    memcpy(m_proj, proj, sizeof(m_proj));

    if (!m_batch.empty())
    {
        glUseProgram(m_shader);
        glUniformMatrix4fv(m_uProj, 1, GL_FALSE, proj);
        glUniform1i(m_uStencilMode, 0);
        glBindVertexArray(m_vao);

        glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
        glBufferData(GL_ARRAY_BUFFER, m_batch.size() * sizeof(Instance),
                     m_batch.data(), GL_DYNAMIC_DRAW);

        glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                (void *)0, (GLsizei)m_batch.size());
        m_batch.clear();
    }

#ifdef MORPH_FEATURE_TEXT
    if (!m_textBatches.empty())
    {
        glUseProgram(m_textShader);
        glUniformMatrix4fv(m_textUProj, 1, GL_FALSE, proj);
        glUniform1i(m_textUAtlas, 0);
        glUniform1i(m_textUColorAtlas, 1);
        glBindVertexArray(m_textVAO);

        for (auto &[key, batch] : m_textBatches)
        {
            if (batch.empty())
                continue;

            // Find the atlas for this key (regular)
            auto it = m_atlases.find(key);
            if (it == m_atlases.end())
                continue;

            // Bind R8 atlas to unit 0
            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, it->second.textureR8);

            // Bind RGBA atlas: use emoji atlas if available, else same atlas
            GLuint colorTex = 0;
            int fs = it->second.fontSize;
            auto emojiIt = m_emojiAtlases.find(fs);
            if (emojiIt != m_emojiAtlases.end() && emojiIt->second.textureRGBA)
                colorTex = emojiIt->second.textureRGBA;
            else if (it->second.textureRGBA)
                colorTex = it->second.textureRGBA;

            glActiveTexture(GL_TEXTURE1);
            glBindTexture(GL_TEXTURE_2D, colorTex);

            glBindBuffer(GL_ARRAY_BUFFER, m_textInstVBO);
            glBufferData(GL_ARRAY_BUFFER, batch.size() * sizeof(TextInstance),
                         batch.data(), GL_DYNAMIC_DRAW);

            glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                    (void *)0, (GLsizei)batch.size());
        }
        m_textBatches.clear();
    }
#endif

#ifdef MORPH_FEATURE_IMAGE
    if (!m_imageBatches.empty())
    {
        glUseProgram(m_imageShader);
        glUniformMatrix4fv(m_imageUProj, 1, GL_FALSE, proj);
        glUniform1i(m_imageUTexture, 0);
        glUniform1i(m_imageUStencil, 0);
        glBindVertexArray(m_imageVAO);

        for (auto &[texId, batch] : m_imageBatches)
        {
            if (batch.empty() || !texId)
                continue;

            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, texId);

            glBindBuffer(GL_ARRAY_BUFFER, m_imageInstVBO);
            glBufferData(GL_ARRAY_BUFFER, batch.size() * sizeof(ImageInstance),
                         batch.data(), GL_DYNAMIC_DRAW);

            glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                    (void *)0, (GLsizei)batch.size());
        }
        m_imageBatches.clear();
    }
#endif

    // Border ring batch — drawn last so it's on top of everything
    if (!m_borderBatch.empty())
    {
        glUseProgram(m_shader);
        glUniformMatrix4fv(m_uProj, 1, GL_FALSE, proj);
        glUniform1i(m_uStencilMode, 0);
        glBindVertexArray(m_vao);

        glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
        glBufferData(GL_ARRAY_BUFFER, m_borderBatch.size() * sizeof(Instance),
                     m_borderBatch.data(), GL_DYNAMIC_DRAW);

        glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                (void *)0, (GLsizei)m_borderBatch.size());
        m_borderBatch.clear();
    }

    glBindVertexArray(0);

#ifdef MORPH_FEATURE_TEXT
    for (auto &[_, a] : m_atlases)
    {
        a.r8Pixels.clear();
        a.r8Pixels.shrink_to_fit();
    }
    for (auto &[_, a] : m_emojiAtlases)
    {
        a.rgbaPixels.clear();
        a.rgbaPixels.shrink_to_fit();
    }
#endif
}
