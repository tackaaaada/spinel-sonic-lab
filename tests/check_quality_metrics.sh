#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() {
    printf '%s\n' "$1" >&2
    exit 1
}

grades() {
    jq -s -f scripts/quality-metrics.jq tests/quality_code.json tests/quality_shell.json |
        jq -r '.[] | "\(.function):\(.severity)"' | sort
}

# A report that measures only one of the fixture's two functions.
partial_report() {
    printf '[{"file":"tests/quality_fixtures/two_functions.rs","function":"one"}]\n'
}

full_report() {
    jq '. + [{"file":"tests/quality_fixtures/two_functions.rs","function":"two"}]'
}

expected=$'cognitive_fail:FAIL\ncognitive_warn:WARN\ncycle_fail:FAIL\ncycle_warn:WARN\nlong:FAIL\nok:OK\nshell_estimate:WARN\nshell_ok:OK'
actual="$(grades)"
[[ "$actual" == "$expected" ]] || fail "Unexpected quality grades:"$'\n'"$actual"

fixture=tests/quality_fixtures/two_functions.rs
if bash scripts/check-quality.sh coverage <(partial_report) "$fixture" >/dev/null; then
    fail "Coverage check accepted a function the metrics tool skipped"
fi
bash scripts/check-quality.sh coverage <(partial_report | full_report) "$fixture" >/dev/null ||
    fail "Coverage check rejected a fully measured file"
