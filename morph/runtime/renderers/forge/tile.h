// renderers/forge/tile.h
#pragma once

struct TileKey {
    int parentLayerId;
    int x, y, w, h;
};

struct Tile {
    TileKey key;
    GLuint texture;
    uint64_t epoch;
    bool opaque = false;
    bool valid = false;
};
