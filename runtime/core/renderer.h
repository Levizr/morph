#pragma once
#include <string>

enum class TextAlign { Left, Center, Right };

class Renderer {
public:
    virtual void clear() = 0;
    virtual void drawRect(float x, float y, float w, float h,
                          float color[4]) = 0;
    virtual void drawRoundedRect(float x, float y, float w, float h,
                                  float radius, float color[4]) {}
    virtual void drawBorderedRect(float x, float y, float w, float h,
                                  float color[4], float borderWidth,
                                  float borderColor[4]) {}
    virtual void drawBorderedRoundedRect(float x, float y, float w, float h,
                                         float radius, float color[4],
                                         float borderWidth,
                                         float borderColor[4]) {}
    virtual void drawText(const std::string& text,
                          float x, float y, float color[4],
                          TextAlign align = TextAlign::Left,
                          float fontSize = 16,
                          const std::string& fontWeight = "normal") {}
    virtual float measureTextWidth(const std::string& text,
                                    float fontSize,
                                    const std::string& fontWeight = "normal") { return 0; }
    virtual void beginClip(float x, float y, float w, float h) {}
    virtual void endClip() {}
    virtual void beginRoundedClip(float x, float y, float w, float h, float radius) {}
    virtual void endRoundedClip() {}

    virtual void drawTexture(unsigned int tex,
                             float x, float y, float w, float h) = 0;
    virtual void drawMesh(const float* verts, const unsigned int* idx,
                          int count, float color[4],
                          float x, float y, float size) = 0;

    void pushScrollOffset(float dx, float dy) { m_scrollX += dx; m_scrollY += dy; }
    void popScrollOffset(float dx, float dy) { m_scrollX -= dx; m_scrollY -= dy; }
    float scrollX() const { return m_scrollX; }
    float scrollY() const { return m_scrollY; }

    int fbHeight() const { return m_fbHeight; }
    void setFBHeight(int h) { m_fbHeight = h; }

    float deltaTime() const { return m_dt; }
    float mouseX()    const { return m_mx; }
    float mouseY()    const { return m_my; }

protected:
    float m_dt = 0.0f, m_mx = 0.0f, m_my = 0.0f;
    int m_fbHeight = 0;
    float m_scrollX = 0.0f, m_scrollY = 0.0f;
};
