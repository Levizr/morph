#pragma once
#include <string>

enum class TextAlign { Left, Center, Right };

class Renderer {
public:
    virtual void clear() = 0;
    virtual void drawRect(float x, float y, float w, float h,
                          float color[4]) = 0;
    virtual void drawRoundedRect(float x, float y, float w, float h,
                                  float radius, float color[4]) = 0;
    virtual void drawText(const std::string& text,
                          float x, float y, float color[4],
                          TextAlign align = TextAlign::Left,
                          float fontSize = 16,
                          const std::string& fontWeight = "normal") = 0;
    virtual void drawTexture(unsigned int tex,
                             float x, float y, float w, float h) = 0;
    virtual void drawMesh(const float* verts, const unsigned int* idx,
                          int count, float color[4],
                          float x, float y, float size) = 0;

    float deltaTime() const { return m_dt; }
    float mouseX()    const { return m_mx; }
    float mouseY()    const { return m_my; }

protected:
    float m_dt = 0.0f, m_mx = 0.0f, m_my = 0.0f;
};
