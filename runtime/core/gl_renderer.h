#pragma once
#include <GLFW/glfw3.h>
#include <cstring>
#include "renderer.h"

class GLRenderer : public Renderer {
public:
    void clear() override {
        glClear(GL_COLOR_BUFFER_BIT | GL_DEPTH_BUFFER_BIT);
    }

    void drawRect(float x, float y, float w, float h, float color[4]) override {
        glColor4f(color[0], color[1], color[2], color[3]);
        glBegin(GL_TRIANGLE_FAN);
        glVertex2f(x,     y);
        glVertex2f(x + w, y);
        glVertex2f(x + w, y + h);
        glVertex2f(x,     y + h);
        glEnd();
    }

    void drawRoundedRect(float x, float y, float w, float h,
                         float radius, float color[4]) override {
        // Fall back to rect for now (TODO: SDF shader)
        drawRect(x, y, w, h, color);
    }

    void drawText(const std::string& text, float x, float y,
                  float color[4], TextAlign align) override {
        // TODO: text rendering with FreeType
        (void)text; (void)x; (void)y; (void)color; (void)align;
    }

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
};
