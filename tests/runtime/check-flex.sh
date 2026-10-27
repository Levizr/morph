#!/usr/bin/env bash
# Flex layout regression test (headless): drives the real MorphNode::layout
# through grow, shrink, space distribution and margins. Guards the invariants
# that keep auto-sized flex containers from overflowing their parent.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/flex-layout-test.XXXXXX")"
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
    "$R/core/tests/flex_layout_test.cpp" \
    "$R/core/node/node.cpp" \
    "$R/core/node/style.cpp" \
    "$R/core/node/animation.cpp" \
    "$R/core/node/layout.cpp" \
    "$R/core/node/events.cpp" \
    "$R/core/node/flatten.cpp" \
    "$R/core/node/paint_order.cpp" \
    -o "$WORK/flex-layout-test" \
    2>"$WORK/build.log" || {
    tail -n 20 "$WORK/build.log"
    echo "[flex-layout-test] BUILD FAILED"
    exit 1
}
"$WORK/flex-layout-test"
