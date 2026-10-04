#!/usr/bin/env bash
# Forge benchmark (Phase 9): damage-model scenarios plus object-level
# single-renderer checks. Measures what is measurable headless:
#   1. Damage-model benchmark (real DamageSet/TilePool/scroll-shift code):
#      100 / 5,000 / 20,000-node scenes x {static, scrub, scroll, full-anim}
#      at 1920x1080 — raster area, present bytes, tile residency.
#   2. Renderer TU sizes: flash.o vs forge.o set (production opt flags).
#   3. TU-exclusion checks via nm: flash window.o references no forge
#      symbols and vice versa (the Phase 8 elimination mechanism).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
CC="${CC:-cc}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/forge-bench.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
OUT="$WORK/forge-bench"
mkdir -p "$WORK"

INC="-I$ROOT/runtime/cpp -I$ROOT/runtime/cpp/renderers -I$ROOT/runtime/cpp/vendor"
"$CC" -O1 -I "$ROOT/runtime/cpp/vendor" \
    -c "$ROOT/runtime/cpp/vendor/glad/glad.c" -o "$WORK/glad.o"
"$CXX" -std=c++23 -O2 $INC \
    "$ROOT/runtime/cpp/renderers/forge/tests/forge_bench.cpp" \
    "$ROOT/runtime/cpp/renderers/forge/damage.cpp" \
    "$ROOT/runtime/cpp/renderers/forge/tile_pool.cpp" \
    "$ROOT/runtime/cpp/renderers/forge/scroll_shift.cpp" \
    "$WORK/glad.o" -ldl -o "$OUT"
"$OUT"

FEATURES="-DMORPH_FEATURE_SCROLL -DMORPH_FEATURE_RADIUS -DMORPH_FEATURE_TEXT -DMORPH_FEATURE_BOLD -DMORPH_FEATURE_POSITION -DMORPH_FEATURE_ZINDEX -DMORPH_FEATURE_OPACITY -DMORPH_FEATURE_FLEX -DMORPH_FEATURE_CURSOR -DMORPH_FEATURE_BORDER -DMORPH_FEATURE_DISPLAY_NONE -DMORPH_FEATURE_INLINE -DMORPH_FEATURE_MARGIN_COLLAPSE -DMORPH_FEATURE_MIN_MAX -DMORPH_FEATURE_BORDER_BOX -DMORPH_FEATURE_IMAGE -DMORPH_FEATURE_INPUT -DMORPH_FEATURE_DIRTY_RENDERING -DMORPH_FEATURE_TRANSFORM -DMORPH_FEATURE_GRADIENT -DMORPH_FEATURE_ANIMATION -DMORPH_FEATURE_REACTIVITY -DMORPH_FEATURE_TASKS -DMORPH_FEATURE_NET -DMORPH_FEATURE_OWNERSHIP -DMORPH_FEATURE_PAGECACHE -DMORPH_FEATURE_HARFBUZZ -DMORPH_FEATURE_HOVER"
SYSINC="-I/usr/include/freetype2 -I/usr/include/libpng16 -I$ROOT/runtime/cpp/harfbuzz"
R="$ROOT/runtime/cpp"

# Production TU sizes: flash (-Oz) vs forge set (-O2), matching opt_flag.
"$CXX" -std=c++23 -Oz $INC $SYSINC $FEATURES \
    -c "$R/renderers/flash/flash.cpp" -o "$WORK/bench-flash.o"
"$CXX" -std=c++23 -O2 $INC $SYSINC $FEATURES -DMORPH_RENDERER_FORGE \
    -c "$R/renderers/forge/forge.cpp" -o "$WORK/bench-forge.o"
"$CXX" -std=c++23 -O2 $INC $SYSINC $FEATURES -DMORPH_RENDERER_FORGE \
    -c "$R/renderers/forge/tile_pool.cpp" -o "$WORK/bench-tile-pool.o"
"$CXX" -std=c++23 -O2 $INC $SYSINC $FEATURES -DMORPH_RENDERER_FORGE \
    -c "$R/renderers/forge/scroll_shift.cpp" -o "$WORK/bench-scroll-shift.o"
"$CXX" -std=c++23 -O2 $INC $SYSINC $FEATURES -DMORPH_RENDERER_FORGE \
    -c "$R/renderers/forge/damage.cpp" -o "$WORK/bench-damage.o"
echo "[forge-bench] TU bytes:"
stat -c "  %n %s" "$WORK/bench-flash.o" "$WORK/bench-forge.o" \
    "$WORK/bench-tile-pool.o" "$WORK/bench-scroll-shift.o" \
    "$WORK/bench-damage.o"

# TU-exclusion: window.o must not reference the dropped backend.
"$CXX" -std=c++23 -Oz $INC $SYSINC $FEATURES \
    -c "$R/core/window.cpp" -o "$WORK/bench-window-flash.o"
"$CXX" -std=c++23 -O2 $INC $SYSINC $FEATURES -DMORPH_RENDERER_FORGE \
    -c "$R/core/window.cpp" -o "$WORK/bench-window-forge.o"
fail=0
# NOTE: nm output goes to temp files first — piping nm straight into
# grep -q races SIGPIPE under `set -o pipefail` (grep exits early).
nm "$WORK/bench-window-flash.o" > "$WORK/nm-flash.txt"
nm "$WORK/bench-window-forge.o" > "$WORK/nm-forge.txt"
nm "$WORK/bench-forge.o" > "$WORK/nm-forge-tu.txt"
if grep -q "U .*forge::" "$WORK/nm-flash.txt"; then
    echo "[forge-bench] FAIL: flash window.o references forge symbols"
    fail=1
else
    echo "[forge-bench] flash window.o references no forge symbols"
fi
if grep -q "U .*flash::" "$WORK/nm-forge.txt"; then
    echo "[forge-bench] FAIL: forge window.o references flash symbols"
    fail=1
else
    echo "[forge-bench] forge window.o references no flash symbols"
fi
if grep -q " T .*forgeCommit" "$WORK/nm-forge-tu.txt"; then
    echo "[forge-bench] forge.o defines forgeCommit"
else
    echo "[forge-bench] FAIL: forge.o missing forgeCommit"
    fail=1
fi
if [ "$fail" -ne 0 ]; then
    echo "FORGE BENCH FAILED"
    exit 1
fi
echo "[forge-bench] all checks passed"
