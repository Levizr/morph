#!/usr/bin/env bash
# Runtime self-tests: rebuild each fixture from scratch and run its
# headless `--morph-self-test` (no display needed — assertions execute
# before GLFW init). Exit non-zero on any failure.
#
# Usage: ./tests/runtime/run-selftests.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
MORPH="$ROOT/target/debug/morph"
if [ ! -x "$MORPH" ]; then
  echo "building morph CLI..."
  (cd "$ROOT" && cargo build -p morphc)
fi

pass=0
fail=0
for fixture in tests/runtime/component-test tests/runtime/native-interop tests/runtime/window-test tests/runtime/route-test; do
  name="$(basename "$fixture")"
  echo "=== $name ==="
  (cd "$ROOT/$fixture" && rm -f .morph/output/"$name"* && "$MORPH" build --no-upx > /dev/null)
  bin="$(cd "$ROOT/$fixture" && find .morph/output -maxdepth 1 -type f -executable | head -1)"
  out="$(cd "$ROOT/$fixture" && "$bin" --morph-self-test)"
  echo "$out"
  if echo "$out" | grep -q "0 failures"; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    echo "SELF-TEST FAILED: $name"
  fi
done
echo "=== $pass passed, $fail failed ==="
[ "$fail" -eq 0 ]
