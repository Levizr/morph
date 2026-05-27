#include "gl_renderer.h"

#ifdef MORPH_FEATURE_IMAGE
void GLRenderer::createImageBuffers() {
    glGenVertexArrays(1, &m_imageVAO);
    glBindVertexArray(m_imageVAO);

    glBindBuffer(GL_ARRAY_BUFFER, m_vbo);
    glVertexAttribPointer(0, 2, GL_FLOAT, GL_FALSE, 0, (void*)0);
    glEnableVertexAttribArray(0);

    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER, m_ibo);

    glGenBuffers(1, &m_imageInstVBO);
    glBindBuffer(GL_ARRAY_BUFFER, m_imageInstVBO);
    glBufferData(GL_ARRAY_BUFFER, 0, nullptr, GL_DYNAMIC_DRAW);

    glEnableVertexAttribArray(1);
    glVertexAttribPointer(1, 4, GL_FLOAT, GL_FALSE, sizeof(ImageInstance), (void*)offsetof(ImageInstance, x));
    glVertexAttribDivisor(1, 1);

    glEnableVertexAttribArray(2);
    glVertexAttribPointer(2, 4, GL_FLOAT, GL_FALSE, sizeof(ImageInstance), (void*)offsetof(ImageInstance, u1));
    glVertexAttribDivisor(2, 1);

    glEnableVertexAttribArray(3);
    glVertexAttribPointer(3, 4, GL_FLOAT, GL_FALSE, sizeof(ImageInstance), (void*)offsetof(ImageInstance, tintR));
    glVertexAttribDivisor(3, 1);

    glBindVertexArray(0);
}
#endif

void GLRenderer::createQuadBuffers() {
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

    glEnableVertexAttribArray(4);
    glVertexAttribPointer(4, 1, GL_FLOAT, GL_FALSE, sizeof(Instance), (void*)offsetof(Instance, borderWidth));
    glVertexAttribDivisor(4, 1);

    glEnableVertexAttribArray(5);
    glVertexAttribPointer(5, 4, GL_FLOAT, GL_FALSE, sizeof(Instance), (void*)offsetof(Instance, br));
    glVertexAttribDivisor(5, 1);

    glEnableVertexAttribArray(6);
    glVertexAttribPointer(6, 1, GL_FLOAT, GL_FALSE, sizeof(Instance), (void*)offsetof(Instance, borderOnly));
    glVertexAttribDivisor(6, 1);

    glBindVertexArray(0);
}

#ifdef MORPH_FEATURE_TEXT
void GLRenderer::createTextBuffers() {
    glGenVertexArrays(1, &m_textVAO);
    glBindVertexArray(m_textVAO);

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

const char* GLRenderer::fontPathForWeight(const std::string& weight) {
    if (weight == "bold" || weight == "700" || weight == "800" || weight == "900")
        return kDefaultFontBold;
    return kDefaultFont;
}

std::string GLRenderer::atlasKey(int fontSize, const std::string& fontWeight) {
    return fontWeight + ":" + std::to_string(fontSize);
}

GLRenderer::FontAtlas& GLRenderer::getOrCreateAtlas(int fontSize, const std::string& fontWeight) {
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

    int totalW = 0, maxH = 0;
    for (char c = 32; c < 127; c++) {
        FT_Load_Char(face, c, FT_LOAD_DEFAULT);
        int adv = (int)(face->glyph->advance.x / 64);
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

        FT_Load_Char(face, c, FT_LOAD_DEFAULT);
        gi.ax = (float)face->glyph->advance.x / 64.0f;
        gi.ay = (float)face->glyph->advance.y / 64.0f;

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
            gi.gw = 0; gi.gh = 0;
            gi.u1 = gi.v1 = gi.u2 = gi.v2 = 0;
        }

        atlas.glyphs[c] = gi;
        if (cx >= texW) break;
    }

    FT_Done_Face(face);

    glGenTextures(1, &atlas.texture);
    glBindTexture(GL_TEXTURE_2D, atlas.texture);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_R8, texW, texH, 0, GL_RED, GL_UNSIGNED_BYTE, data.data());
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

GLRenderer::~GLRenderer() {
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
#ifdef MORPH_FEATURE_IMAGE
    if (m_imageVAO) glDeleteVertexArrays(1, &m_imageVAO);
    if (m_imageInstVBO) glDeleteBuffers(1, &m_imageInstVBO);
    if (m_imageShader) glDeleteProgram(m_imageShader);
    for (auto& [_, pair] : m_textureCache)
        if (pair.first) glDeleteTextures(1, &pair.first);
#endif
}

bool GLRenderer::ensureReady() {
    if (m_ready) return true;

    createProgram(kQuadVertSrc, kQuadFragSrc, m_shader, m_uProj);
    m_uStencilMode = glGetUniformLocation(m_shader, "uStencilMode");
    createQuadBuffers();

#ifdef MORPH_FEATURE_TEXT
    if (FT_Init_FreeType(&m_ft)) {
        fprintf(stderr, "[GLRenderer] failed to init FreeType\n");
    }
    createProgram(kTextVertSrc, kTextFragSrc, m_textShader, m_textUProj);
    m_textUAtlas = glGetUniformLocation(m_textShader, "uAtlas");
    createTextBuffers();
#endif

#ifdef MORPH_FEATURE_IMAGE
    createProgram(kImageVertSrc, kImageFragSrc, m_imageShader, m_imageUProj);
    m_imageUTexture = glGetUniformLocation(m_imageShader, "uTexture");
    m_imageUStencil = glGetUniformLocation(m_imageShader, "uStencilMode");
    createImageBuffers();
#endif

    glClearColor(1,1,1,1);
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

void GLRenderer::clear() {
    if (!ensureReady()) return;
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT | GL_STENCIL_BUFFER_BIT);
}

void GLRenderer::beginClip(float x, float y, float w, float h) {
    float cx = x + m_scrollX;
    float cy = y + m_scrollY;
    glEnable(GL_SCISSOR_TEST);
    glScissor((GLint)cx, m_fbHeight - (GLint)(cy + h), (GLsizei)w, (GLsizei)h);
}

void GLRenderer::endClip() {
    glDisable(GL_SCISSOR_TEST);
}

void GLRenderer::beginRoundedClip(float x, float y, float w, float h, float radius) {
    flush(m_proj);

    // Use GL_INCR so nested masks properly intersect:
    //   Level 1: stencil goes 0→1 for fragments inside shape 1.
    //   Level 2: stencil goes 1→2 for fragments inside BOTH shape 1 AND shape 2.
    // Because discard in the shader prevents INCR for fragments outside the shape,
    // only the intersection of all ancestor masks plus this shape passes.
    glEnable(GL_STENCIL_TEST);
    glStencilFunc(GL_ALWAYS, 0xFF, 0xFF);
    glStencilOp(GL_KEEP, GL_KEEP, GL_INCR);
    glColorMask(GL_FALSE, GL_FALSE, GL_FALSE, GL_FALSE);
    glDepthMask(GL_FALSE);

    glUseProgram(m_shader);
    glUniformMatrix4fv(m_uProj, 1, GL_FALSE, m_proj);
    glUniform1i(m_uStencilMode, 1);

    Instance inst = {x + m_scrollX, y + m_scrollY, w, h,
                     1.0f, 1.0f, 1.0f, 1.0f,
                     radius, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 0.0f};
    glBindVertexArray(m_vao);
    glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
    glBufferData(GL_ARRAY_BUFFER, sizeof(Instance), &inst, GL_DYNAMIC_DRAW);
    glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT, (void*)0, 1);

    glUniform1i(m_uStencilMode, 0);
    glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);
    glDepthMask(GL_TRUE);

    int newRef = m_stencilClipDepth + 1;
    glStencilFunc(GL_EQUAL, newRef, 0xFF);
    glStencilOp(GL_KEEP, GL_KEEP, GL_KEEP);

    m_stencilClipDepth++;
}

void GLRenderer::endRoundedClip() {
    flush(m_proj);
    m_stencilClipDepth--;
    if (m_stencilClipDepth > 0) {
        glStencilFunc(GL_EQUAL, m_stencilClipDepth, 0xFF);
    } else {
        glDisable(GL_STENCIL_TEST);
        m_stencilClipDepth = 0;
    }
}

void GLRenderer::flush(const float proj[16]) {
    if (!ensureReady()) return;

    memcpy(m_proj, proj, sizeof(m_proj));

    if (!m_batch.empty()) {
        glUseProgram(m_shader);
        glUniformMatrix4fv(m_uProj, 1, GL_FALSE, proj);
        glUniform1i(m_uStencilMode, 0);
        glBindVertexArray(m_vao);

        glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
        glBufferData(GL_ARRAY_BUFFER, m_batch.size() * sizeof(Instance),
                     m_batch.data(), GL_DYNAMIC_DRAW);

        glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                (void*)0, (GLsizei)m_batch.size());
        m_batch.clear();
    }

#ifdef MORPH_FEATURE_TEXT
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

#ifdef MORPH_FEATURE_IMAGE
    if (!m_imageBatches.empty()) {
        glUseProgram(m_imageShader);
        glUniformMatrix4fv(m_imageUProj, 1, GL_FALSE, proj);
        glUniform1i(m_imageUTexture, 0);
        glUniform1i(m_imageUStencil, 0);
        glBindVertexArray(m_imageVAO);

        for (auto& [texId, batch] : m_imageBatches) {
            if (batch.empty() || !texId) continue;

            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, texId);

            glBindBuffer(GL_ARRAY_BUFFER, m_imageInstVBO);
            glBufferData(GL_ARRAY_BUFFER, batch.size() * sizeof(ImageInstance),
                         batch.data(), GL_DYNAMIC_DRAW);

            glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                    (void*)0, (GLsizei)batch.size());
        }
        m_imageBatches.clear();
    }
#endif

    // Border ring batch — drawn last so it's on top of everything
    if (!m_borderBatch.empty()) {
        glUseProgram(m_shader);
        glUniformMatrix4fv(m_uProj, 1, GL_FALSE, proj);
        glUniform1i(m_uStencilMode, 0);
        glBindVertexArray(m_vao);

        glBindBuffer(GL_ARRAY_BUFFER, m_instVBO);
        glBufferData(GL_ARRAY_BUFFER, m_borderBatch.size() * sizeof(Instance),
                     m_borderBatch.data(), GL_DYNAMIC_DRAW);

        glDrawElementsInstanced(GL_TRIANGLES, 6, GL_UNSIGNED_INT,
                                (void*)0, (GLsizei)m_borderBatch.size());
        m_borderBatch.clear();
    }

    glBindVertexArray(0);
}
