#!/usr/bin/env bash
# Net runtime test (headless): Response.json() must parse the body with the
# same JSON DOM parser as JSON.parse, not return a raw JsString.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CXX="${CXX:-g++-14}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/net-test.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

R="$ROOT/runtime/cpp"
SRC="$R/core/tests/net_test.cpp"

"$CXX" -std=c++23 -O1 \
    -I "$R" \
    "$SRC" \
    -o "$WORK/net-test" \
    2>"$WORK/build.log" || {
    tail -n 20 "$WORK/build.log"
    exit 1
}
"$WORK/net-test"

echo "✓ net-json: OK"
