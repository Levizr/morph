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
    cout="$(cd "$dir" && "$MORPH" check 2>&1)" || true
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
run_pass pass-js-types
run_pass pass-arrow-default
run_pass pass-export-spellings
run_pass pass-const-inits
run_pass pass-async
run_pass pass-obj-spread
run_pass pass-destructuring
run_pass pass-destructuring-assignments
run_pass pass-per-iteration-closures
run_pass pass-array-methods
run_pass pass-operators
run_pass pass-math
run_pass pass-window
run_pass pass-late-values
run_pass pass-control-flow
run_pass pass-array-for-in
run_pass pass-new-target
run_pass pass-hoisting
run_pass pass-module-side-effects
run_pass pass-json-dot
run_pass pass-then-chain
run_pass pass-timer-args
run_pass pass-string-switch
run_pass pass-widening
run_pass pass-classes
run_pass pass-rule-warnings
run_pass pass-lint-config

run_fail fail-math-member mx-js-member
run_fail fail-window-member mx-window-member
run_fail fail-typeof-op mx-js-op
run_fail fail-instanceof-op mx-js-op
run_fail fail-generator mx-js-syntax
run_fail fail-with-stmt mx-js-syntax
run_fail fail-labeled-break mx-js-syntax
run_fail fail-import-meta mx-js-syntax
run_fail fail-dyn-import mx-js-syntax
run_fail fail-namespace mx-js-syntax
run_fail fail-namespace-import mx-js-syntax
run_fail fail-call-spread mx-js-syntax
run_fail fail-str-method mx-js-method
run_fail fail-document-global mx-js-global
run_fail fail-state-scope mx-state-scope
run_fail fail-undefined-name mx-undefined
run_fail fail-missing-css mx-css-file-missing
run_fail fail-inline-style mx-style-prop
run_fail fail-state-init mx-state-init
run_fail fail-state-pattern mx-state-pattern
run_fail fail-class-expr-position mx-js-syntax
run_fail fail-effect-nested mx-effect-nested
run_fail fail-img-src mx-img-src
run_fail fail-event-value mx-event-value
run_fail fail-windowconfig-key mx-windowconfig-key
run_fail fail-windowconfig-type mx-windowconfig-type
run_fail fail-import-morph mx-import-morph
run_fail fail-import-type mx-import-type
run_fail fail-effect-cb mx-effect-cb
run_fail fail-object-fn mx-js-syntax
run_fail fail-for-in-pattern mx-js-syntax
run_fail fail-then-chain mx-js-syntax
run_fail fail-then-chain-value mx-js-syntax
run_fail fail-tdz mx-js-syntax
run_fail fail-json-args mx-js-syntax
run_fail fail-timer-args-short mx-js-syntax
run_fail fail-lint-severity mx-key-misuse

echo "[conformance] $checks checks, $failures failures"
[ "$failures" -eq 0 ]
