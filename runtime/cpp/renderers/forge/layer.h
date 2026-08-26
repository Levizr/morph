// renderers/forge/layer.h
#pragma once

struct RetainedLayer {
    int nodeId;
    GLuint texture;
    int w, h;
    bool active;
};
