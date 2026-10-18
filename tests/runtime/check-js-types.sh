#!/usr/bin/env bash
# JS runtime types test: number formatting (toFixed/toPrecision/
# toExponential/toString), String/Number/Boolean conversions, parseInt/
# parseFloat, Number predicates, Array/Object methods, Math and JSON.
# Expected values are pinned in runtime/cpp/core/tests/js_types_test.cpp
# and were verified against node v24. Semantics gate the checker's
# mx-js-member allowlist, so this runs before fixtures that use them.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/js-types-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

R="$ROOT/runtime/cpp"
SRC="$R/core/tests/js_types_test.cpp"

# Default (exceptions on): includes the RangeError/SyntaxError throw paths.
"$CXX" -std=c++23 -O1 \
    -I "$R" \
    -I "$R/vendor" \
    "$SRC" \
    -o "$WORK/js-types-test" \
    2>"$WORK/build.log" || {
    tail -n 20 "$WORK/build.log"
    exit 1
}
"$WORK/js-types-test"

# -fno-exceptions (the app default): the headers must still compile and
# every non-throwing path must pass.
"$CXX" -std=c++23 -O1 -fno-exceptions \
    -I "$R" \
    -I "$R/vendor" \
    "$SRC" \
    -o "$WORK/js-types-test-noexc" \
    2>"$WORK/build-noexc.log" || {
    tail -n 20 "$WORK/build-noexc.log"
    exit 1
}
"$WORK/js-types-test-noexc"

echo "✓ js-types: OK"
