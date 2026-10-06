#!/usr/bin/env bash
# Border hover/active/transition/keyframe state test: drives the real
# MorphNode state machinery headless (no window, no EGL) against
# per-side border widths/colors, corner radii, border-image gradients
# and the BorderColor/BorderWidth/BorderGradient keyframe properties.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/border-state-test.XXXXXX")"
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
    "$R/core/tests/border_state_test.cpp" \
    "$R/core/node/node.cpp" \
    "$R/core/node/style.cpp" \
    "$R/core/node/animation.cpp" \
    "$R/core/node/layout.cpp" \
    "$R/core/node/events.cpp" \
    "$R/core/node/flatten.cpp" \
    "$R/core/node/paint_order.cpp" \
    -o "$WORK/border-state-test" \
    2>"$WORK/build.log" || {
    tail -n 20 "$WORK/build.log"
    echo "[border-state-test] BUILD FAILED"
    exit 1
}
"$WORK/border-state-test"
