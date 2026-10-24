#!/usr/bin/env bash
# Responsive @media variant selection over the real MorphNode style machinery.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/responsive-style-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
R="$ROOT/runtime/cpp"
"$CXX" -std=c++23 -O1 \
    -I "$R" -I "$R/vendor" \
    -DMORPH_FEATURE_MEDIA \
    -DMORPH_FEATURE_BORDER \
    -DMORPH_FEATURE_OUTLINE \
    -DMORPH_FEATURE_TEXT_SHADOW \
    -DMORPH_FEATURE_POSITION \
    -DMORPH_FEATURE_FLEX \
    -DMORPH_FEATURE_SCROLL \
    -DMORPH_FEATURE_OPACITY \
    -DMORPH_FEATURE_TRANSFORM \
    -DMORPH_FEATURE_CURSOR \
    -DMORPH_FEATURE_ZINDEX \
    "$R/core/tests/responsive_style_test.cpp" \
    "$R/core/node/node.cpp" \
    "$R/core/node/style.cpp" \
    "$R/core/node/animation.cpp" \
    "$R/core/node/layout.cpp" \
    "$R/core/node/events.cpp" \
    "$R/core/node/flatten.cpp" \
    "$R/core/node/paint_order.cpp" \
    -o "$WORK/responsive-style-test" 2>"$WORK/build.log" || {
    tail -n 30 "$WORK/build.log"
    echo "[responsive-style-test] BUILD FAILED"
    exit 1
}
"$WORK/responsive-style-test"
