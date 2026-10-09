#!/usr/bin/env bash
# Scroll layout test: drives the real MorphNode::layout/event dispatch
# against per-axis overflow enables, wheel chaining (deepest-first,
# remainder bubbles outward), horizontal/shift+wheel, clamps, page
# scroll and scrollbar thumb geometry (headless).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/scroll-layout-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

R="$ROOT/runtime/cpp"
"$CXX" -std=c++23 -O1 \
    -I "$R" \
    -I "$R/vendor" \
    -DMORPH_FEATURE_BORDER \
    -DMORPH_FEATURE_GRADIENT \
    -DMORPH_FEATURE_ANIMATION \
    -DMORPH_FEATURE_TRANSFORM \
    -DMORPH_FEATURE_POSITION \
    -DMORPH_FEATURE_OPACITY \
    -DMORPH_FEATURE_FLEX \
    -DMORPH_FEATURE_SCROLL \
    -DMORPH_FEATURE_CURSOR \
    -DMORPH_FEATURE_ZINDEX \
    -DMORPH_FEATURE_MIN_MAX \
    "$R/core/tests/scroll_layout_test.cpp" \
    "$R/core/node/node.cpp" \
    "$R/core/node/style.cpp" \
    "$R/core/node/animation.cpp" \
    "$R/core/node/layout.cpp" \
    "$R/core/node/events.cpp" \
    "$R/core/node/flatten.cpp" \
    "$R/core/node/paint_order.cpp" \
    -o "$WORK/scroll-layout-test" \
    2>"$WORK/build.log" || {
    tail -n 20 "$WORK/build.log"
    echo "[scroll-layout-test] BUILD FAILED"
    exit 1
}
"$WORK/scroll-layout-test"
