#!/usr/bin/env bash
# Reactive color parser test (headless): transparent and hex forms used by
# generated inline-style effects, including invalid-input non-destruction.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/color-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

R="$ROOT/runtime/cpp"
"$CXX" -std=c++23 -O1 \
    -I "$R" \
    "$R/core/tests/color_test.cpp" \
    -o "$WORK/color-test" \
    2>"$WORK/build.log" || {
    tail -n 20 "$WORK/build.log"
    echo "[color-test] BUILD FAILED"
    exit 1
}
"$WORK/color-test"
