#!/usr/bin/env bash
# Conformance: `morph check` agrees with `morph build` on every fixture.
# PASS fixtures: check reports zero errors AND the build succeeds.
# FAIL fixtures: check reports the expected error code AND the build is
# blocked by the lint gate (not by some unrelated failure).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CONF="$ROOT/tests/runtime/conformance"
MORPH="$ROOT/target/debug/morph"

checks=0
failures=0
check() {
    checks=$((checks + 1))
    if ! eval "$1"; then
        failures=$((failures + 1))
        printf 'FAIL %s\n' "$2"
    fi
}

cargo build -p morphc 2>/dev/null 1>&2 || {
    echo "[conformance] morphc BUILD FAILED"
    exit 1
}

run_pass() {
    local name="$1"
    local dir="$CONF/$name"
    rm -rf "$dir/.morph"
    local cout
    cout="$(cd "$dir" && "$MORPH" check 2>&1)"
    check "! grep -qE '^  error :' <<<\"\$cout\"" "$name check reports zero errors (got: $(grep -E '^  error :' <<<"$cout" | head -n 3))"
    if grep -qE '^  error :' <<<"$cout"; then
        return
    fi
    local bout rc=0
    bout="$(cd "$dir" && "$MORPH" build --no-upx 2>&1)" || rc=$?
    check "[ $rc -eq 0 ]" "$name build succeeds (rc=$rc)"
    rm -rf "$dir/.morph"
}

run_fail() {
    local name="$1"
    local code="$2"
    local dir="$CONF/$name"
    rm -rf "$dir/.morph"
    local cout
    cout="$(cd "$dir" && "$MORPH" check 2>&1)"
    check "grep -q \"error : $code\" <<<\"\$cout\"" "$name check reports $code"
    local bout rc=0
    bout="$(cd "$dir" && "$MORPH" build --no-upx 2>&1)" || rc=$?
    check "[ $rc -ne 0 ]" "$name build fails (rc=$rc)"
    check "grep -q 'build blocked by' <<<\"\$bout\"" "$name build blocked by lint gate"
    rm -rf "$dir/.morph"
}

trap 'rm -rf "$CONF"/*/\.morph' EXIT

run_pass pass-full
run_pass pass-css

run_fail fail-math-member mx-js-member
run_fail fail-pow-op mx-js-op
run_fail fail-typeof-op mx-js-op
run_fail fail-instanceof-op mx-js-op
run_fail fail-generator mx-js-syntax
run_fail fail-with-stmt mx-js-syntax
run_fail fail-obj-spread mx-js-syntax
run_fail fail-labeled-break mx-js-syntax
run_fail fail-import-meta mx-js-syntax
run_fail fail-dyn-import mx-js-syntax
run_fail fail-namespace mx-js-syntax
run_fail fail-array-map mx-js-method
run_fail fail-str-method mx-js-method
run_fail fail-document-global mx-js-global
run_fail fail-state-scope mx-state-scope
run_fail fail-undefined-name mx-undefined
run_fail fail-missing-css mx-css-file-missing
run_fail fail-inline-style mx-style-prop
run_fail fail-destructuring mx-js-syntax
run_fail fail-state-init mx-state-init
run_fail fail-state-pattern mx-state-pattern
run_fail fail-const-init mx-js-syntax

echo "[conformance] $checks checks, $failures failures"
[ "$failures" -eq 0 ]
