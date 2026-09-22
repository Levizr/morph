#!/usr/bin/env bash
# Lean-binary budget gate: hello-size (single h1) must stay <= 150KB
# WITHOUT --self-test and WITHOUT upx. Prints file bytes + UPX size.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MORPH="$ROOT/target/debug/morph"
BUDGET=153600
(cd "$ROOT" && cargo build -q -p morphc)
(cd "$ROOT/tests/runtime/hello-size" && rm -f .morph/output/hello-size* && "$MORPH" build --no-upx > /dev/null)
BIN="$ROOT/tests/runtime/hello-size/.morph/output/hello-size"
SIZE=$(stat -c%s "$BIN")
echo "hello-size: $SIZE bytes (budget $BUDGET)"
if [ "$SIZE" -gt "$BUDGET" ]; then
  echo "SIZE BUDGET EXCEEDED: $SIZE > $BUDGET"
  exit 1
fi
echo "budget OK"
