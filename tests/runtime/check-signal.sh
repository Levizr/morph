#!/usr/bin/env bash
# Signal overload-resolution test (headless): dynamic JsValue writes must use
# the typed value overload, while callable updaters still use the functional
# overload. Guards the text-input stack overflow fixed in signal.h.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/signal-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

R="$ROOT/runtime/cpp"
"$CXX" -std=c++23 -O1 \
    -I "$R" \
    -I "$R/vendor" \
    -DMORPH_FEATURE_REACTIVITY \
    "$R/core/tests/signal_test.cpp" \
    "$R/reactivity/effect.cpp" \
    -o "$WORK/signal-test" \
    2>"$WORK/build.log" || {
    tail -n 20 "$WORK/build.log"
    echo "[signal-test] BUILD FAILED"
    exit 1
}
"$WORK/signal-test"
