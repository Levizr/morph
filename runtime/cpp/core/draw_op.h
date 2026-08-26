#pragma once
#include <cstdint>
#include <cstring>

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
    void setRounded(float _x, float _y, float _w, float _h, float rad, float cr[4])
    {
        type = RoundedRect;
        x = _x; y = _y; w = _w; h = _h;
        r = cr[0]; g = cr[1]; b = cr[2]; a = cr[3];
        data[0] = rad;
        for (int i = 1; i < 6; i++) data[i] = 0;
        br = bg = bb = ba = 0;
        texId = 0;
    }
    void setBordered(float _x, float _y, float _w, float _h, float rad,
                     float cr[4], float bw, float bc[4])
    {
        type = rad > 0 ? BorderedRoundedRect : BorderedRect;
        x = _x; y = _y; w = _w; h = _h;
        r = cr[0]; g = cr[1]; b = cr[2]; a = cr[3];
        data[0] = rad;
        data[1] = bw;
        for (int i = 2; i < 6; i++) data[i] = 0;
        br = bc[0]; bg = bc[1]; bb = bc[2]; ba = bc[3];
        texId = 0;
    }
    void setClip(float _x, float _y, float _w, float _h, bool rounded, float rad)
    {
        type = rounded ? BeginRoundedClip : BeginClip;
        x = _x; y = _y; w = _w; h = _h;
        data[0] = rad;
        for (int i = 1; i < 6; i++) data[i] = 0;
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
