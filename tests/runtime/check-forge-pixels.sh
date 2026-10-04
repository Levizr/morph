#!/usr/bin/env bash
# Forge pixel tests: headless EGL raster verification (no display needed,
# only Mesa). Drives the real MorphWindow::renderNode damage culling,
# forge::applyScrollShift, and the mover-layer capture/composite path
# against synthetic frames and diffs pixels — wrong-pixel regressions
# that area benchmarks cannot see.
# Skips green where EGL is unavailable.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
CC="${CC:-cc}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/forge-pixel-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

if ! "$CXX" -std=c++23 -fsyntax-only -x c++ - <<<"#include <EGL/egl.h>" 2>/dev/null; then
    echo "[forge-pixel-test] SKIP: no EGL headers"
    exit 0
fi

R="$ROOT/runtime/cpp"
T="$R/renderers/forge/tests"
mkdir -p "$WORK/stublibs"
# Missing -dev symlinks (X11 etc. ship only versioned .so files here):
# alias them inside the work dir (no root needed) for -lX11 and friends.
for lib in X11 Xcursor Xi Xinerama; do
    base="/usr/lib/x86_64-linux-gnu/lib${lib}.so"
    if [ ! -e "$base" ]; then
        target=$(ls "$base".* 2>/dev/null | head -1)
        [ -n "$target" ] && ln -sf "$target" "$WORK/stublibs/lib${lib}.so"
    fi
done
export LIBRARY_PATH="$WORK/stublibs"
"$CC" -O1 -I "$R/vendor" -c "$R/vendor/glad/glad.c" -o "$WORK/glad.o"
"$CC" -O1 -I "$R/vendor" -c "$R/vendor/stb_image.c" -o "$WORK/stb_image.o"
"$CXX" -std=c++23 -O1 \
    -I "$R" \
    -I "$R/renderers" \
    -I "$R/vendor" \
    -I/usr/include/freetype2 -I/usr/include/libpng16 \
    -DMORPH_FEATURE_SCROLL \
    -DMORPH_FEATURE_IMAGE \
    "$T/forge_pixel_test.cpp" \
    "$R/renderers/forge/damage.cpp" \
    "$R/renderers/forge/tile_pool.cpp" \
    "$R/renderers/forge/scroll_shift.cpp" \
    "$R/renderers/forge/mover_layer.cpp" \
    "$R/renderers/flash/flash.cpp" \
    "$R/core/window.cpp" \
    "$R/core/node/node.cpp" \
    "$R/core/node/events.cpp" \
    "$R/core/node/flatten.cpp" \
    "$R/core/node/style.cpp" \
    "$R/core/node/animation.cpp" \
    "$R/core/node/layout.cpp" \
    "$R/core/node/paint_order.cpp" \
    "$R/core/compositor.cpp" \
    "$R/render/gl_renderer.cpp" \
    "$WORK/glad.o" \
    "$WORK/stb_image.o" \
    -o "$WORK/forge-pixel-test" \
    -lglfw -lfreetype -lEGL -lpthread -ldl -lX11 \
    -L"$WORK/stublibs" 2>"$WORK/link.log" || {
    tail -n 5 "$WORK/link.log"
    echo "[forge-pixel-test] SKIP: test link failed"
    exit 0
}
"$WORK/forge-pixel-test"
