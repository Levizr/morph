#pragma once
#include <cstdint>
#include <cstring>

inline float maxRadius4(const float r[4])
{
    float m = r[0];
    for (int i = 1; i < 4; i++)
    {
        if (r[i] > m)
        {
            m = r[i];
        }
    }
    return m;
}

struct DrawOp
{
    enum Type : uint8_t
    {
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
    float r, g, b, a;
    float data[6];
    float br, bg, bb, ba;
    // Per-corner radii in clockwise order from top-left (TL TR BR BL).
    // data[0] keeps the max radius so cheap sharp/round checks still work.
    float radii[4] = {0.0f, 0.0f, 0.0f, 0.0f};
    uint32_t texId;

    void setRect(float _x, float _y, float _w, float _h, float cr[4])
    {
        type = Rect;
        x = _x; y = _y; w = _w; h = _h;
        r = cr[0]; g = cr[1]; b = cr[2]; a = cr[3];
        for (int i = 0; i < 6; i++) data[i] = 0;
        br = bg = bb = ba = 0;
        texId = 0;
    }
    void setRounded(float _x, float _y, float _w, float _h, const float rad[4], float cr[4])
    {
        type = RoundedRect;
        x = _x; y = _y; w = _w; h = _h;
        r = cr[0]; g = cr[1]; b = cr[2]; a = cr[3];
        data[0] = maxRadius4(rad);
        for (int i = 1; i < 6; i++) data[i] = 0;
        for (int i = 0; i < 4; i++) radii[i] = rad[i];
        br = bg = bb = ba = 0;
        texId = 0;
    }
    void setBordered(float _x, float _y, float _w, float _h, const float rad[4],
                     float cr[4], float bw, float bc[4], float mode = 0.0f)
    {
        type = (rad[0] > 0.0f || rad[1] > 0.0f || rad[2] > 0.0f || rad[3] > 0.0f) ? BorderedRoundedRect : BorderedRect;
        x = _x; y = _y; w = _w; h = _h;
        r = cr[0]; g = cr[1]; b = cr[2]; a = cr[3];
        data[0] = maxRadius4(rad);
        data[1] = bw;
        data[2] = mode;
        for (int i = 3; i < 6; i++) data[i] = 0;
        for (int i = 0; i < 4; i++) radii[i] = rad[i];
        br = bc[0]; bg = bc[1]; bb = bc[2]; ba = bc[3];
        texId = 0;
    }
    void setBorderRing(float _x, float _y, float _w, float _h, const float rad[4],
                       float bw, float bc[4], float mode = 0.0f,
                       float adjStart = -1.0f, float adjEnd = -1.0f)
    {
        type = BorderRing;
        x = _x; y = _y; w = _w; h = _h;
        r = g = b = a = 0;
        data[0] = maxRadius4(rad);
        data[1] = bw;
        data[2] = mode;
        data[3] = adjStart;
        data[4] = adjEnd;
        data[5] = 0;
        for (int i = 0; i < 4; i++) radii[i] = rad[i];
        br = bc[0]; bg = bc[1]; bb = bc[2]; ba = bc[3];
        texId = 0;
    }
    void setClip(float _x, float _y, float _w, float _h, bool rounded, const float rad[4])
    {
        type = rounded ? BeginRoundedClip : BeginClip;
        x = _x; y = _y; w = _w; h = _h;
        data[0] = maxRadius4(rad);
        for (int i = 1; i < 6; i++) data[i] = 0;
        for (int i = 0; i < 4; i++) radii[i] = rad[i];
        r = g = b = a = br = bg = bb = ba = texId = 0;
    }
    void setEndClip(bool rounded)
    {
        type = rounded ? EndRoundedClip : EndClip;
        x = y = w = h = r = g = b = a = texId = 0;
        for (int i = 0; i < 6; i++) data[i] = 0;
        br = bg = bb = ba = 0;
    }
    void setScroll(float sy, bool push)
    {
        type = push ? PushScroll : PopScroll;
        r = sy;
        x = y = w = h = g = b = a = texId = 0;
        for (int i = 0; i < 6; i++) data[i] = 0;
        br = bg = bb = ba = 0;
    }
};
