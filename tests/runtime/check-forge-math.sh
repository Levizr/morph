#!/usr/bin/env bash
# Forge math unit tests: DamageSet union/clip, TilePool LRU/epoch/budget,
# scroll-shift detection + exposed strip, LayerPool promote/prune.
# Compiles the pure-CPU forge TUs with g++-14 -std=c++23 and runs headless.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
CC="${CC:-cc}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/forge-math-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
BIN="$WORK/forge-math-test"
GLAD_OBJ="$WORK/glad.o"
"$CC" -O1 -I "$ROOT/runtime/cpp/vendor" \
    -c "$ROOT/runtime/cpp/vendor/glad/glad.c" -o "$GLAD_OBJ"
"$CXX" -std=c++23 -O1 \
    -I "$ROOT/runtime/cpp" \
    -I "$ROOT/runtime/cpp/renderers" \
    -I "$ROOT/runtime/cpp/vendor" \
    "$ROOT/runtime/cpp/renderers/forge/tests/forge_math_test.cpp" \
    "$ROOT/runtime/cpp/renderers/forge/damage.cpp" \
    "$ROOT/runtime/cpp/renderers/forge/tile_pool.cpp" \
    "$ROOT/runtime/cpp/renderers/forge/scroll_shift.cpp" \
    "$GLAD_OBJ" -ldl -o "$BIN"
out="$("$BIN" || true)"
echo "$out"
if echo "$out" | grep -q "0 failures"; then
    exit 0
else
    echo "FORGE MATH TESTS FAILED"
    exit 1
fi
