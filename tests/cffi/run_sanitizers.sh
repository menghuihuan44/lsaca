#!/usr/bin/env bash
# Build and run the C FFI acceptance suite under ASan and UBSan.
#
# Usage:
#   tests/cffi/run_sanitizers.sh [path-to-lsaca]
#
# The lsaca binary defaults to target/release/lsaca (the sanitizer CI job
# builds it first). Every positive program in programs/ is lowered to C with
# emit-c, compiled with the sanitizers, run, and compared byte-for-byte with
# its .stdout file. Every negative program in negative/ must be rejected with
# the diagnostic code recorded in expectations.txt.
set -euo pipefail

# Standalone runs still fail on a sanitizer report. The CI job may override
# these through its own ASAN_OPTIONS/UBSAN_OPTIONS.
export ASAN_OPTIONS="${ASAN_OPTIONS:-detect_leaks=1:abort_on_error=1}"
export UBSAN_OPTIONS="${UBSAN_OPTIONS:-halt_on_error=1:print_stacktrace=1}"

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cli="${1:-${LSACA_BIN:-$repo_root/target/release/lsaca}}"
fixtures="$repo_root/crates/lsaca/tests/programs/cffi"
programs="$fixtures"
negative="$fixtures/negative"
expectations="$repo_root/tests/cffi/expectations.txt"

if [[ ! -x "$cli" ]]; then
    echo "cffi: lsaca binary not found or not executable: $cli" >&2
    echo "cffi: build it with: cargo build --release -p lsaca-cli" >&2
    exit 1
fi

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT
failures=0
checks=0

fail() {
    echo "cffi: FAIL: $*" >&2
    failures=$((failures + 1))
}

echo "cffi: lsaca = $cli"
echo "cffi: workdir = $workdir"

for program in "$programs"/*.ls; do
    name="$(basename "$program" .ls)"
    expected_file="${program%.ls}.stdout"
    out="$workdir/positive/$name"
    checks=$((checks + 1))
    if [[ ! -f "$expected_file" ]]; then
        fail "$name: missing expected stdout file $expected_file"
        continue
    fi
    if ! "$cli" emit-c -r -o "$out" "$program"; then
        fail "$name: emit-c failed"
        continue
    fi
    if ! gcc -std=c17 -Wall -Wextra -Werror -O1 -g -fsanitize=address,undefined \
        -fno-omit-frame-pointer -I "$out" "$out"/*.c -lm -o "$out/$name"; then
        fail "$name: sanitizer build failed"
        continue
    fi
    err="$workdir/$name.stderr"
    actual="$workdir/$name.stdout"
    if ! "$out/$name" >"$actual" 2>"$err"; then
        cat "$err" >&2
        fail "$name: program exited non-zero"
        continue
    fi
    if [[ -s "$err" ]]; then
        cat "$err" >&2
        fail "$name: unexpected stderr"
        continue
    fi
    if ! cmp -s "$expected_file" "$actual"; then
        printf '%s\n' "--- expected ---" >&2
        cat "$expected_file" >&2
        printf '%s\n' "--- actual ---" >&2
        cat "$actual" >&2
        fail "$name: stdout mismatch"
        continue
    fi
    echo "cffi: PASS positive $name"
done

while IFS='|' read -r rel kind message <&3; do
    if [[ -z "$rel" || "$rel" == \#* ]]; then
        continue
    fi
    fixture="$fixtures/$rel"
    checks=$((checks + 1))
    set +e
    output="$("$cli" emit-c -r -o "$workdir/negative/$(basename "$rel" .ls)" "$fixture" </dev/null 2>&1)"
    status=$?
    set -e
    if [[ $status -eq 0 ]]; then
        fail "$rel: compiler accepted a program that must be rejected"
        continue
    fi
    if ! grep -qE "(^|[^A-Za-z0-9_])$kind([^A-Za-z0-9_]|$)" <<<"$output"; then
        printf '%s\n' "$output" >&2
        fail "$rel: expected diagnostic $kind, none found"
        continue
    fi
    if [[ -n "$message" ]] && ! grep -qF "$message" <<<"$output"; then
        printf '%s\n' "$output" >&2
        fail "$rel: diagnostic $kind does not contain required text: $message"
        continue
    fi
    echo "cffi: PASS negative $rel ($kind)"
done 3< "$expectations"

if [[ $failures -ne 0 ]]; then
    echo "cffi: FAILED ($failures of $checks checks)" >&2
    exit 1
fi
echo "cffi: PASSED ($checks checks)"
