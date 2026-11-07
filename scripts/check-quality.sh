#!/usr/bin/env bash
# Gate per-function length and complexity as described in docs/quality-policy.md.
#   check-quality.sh                      measure the project and fail on FAIL grades
#   check-quality.sh coverage REPORT FILE...  fail when REPORT skips a declared function
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

declared_functions() {
    case "$1" in
    *.rs) grep -cE '^\s*(pub(\([a-z]+\))? )?(const )?(async )?(unsafe )?fn ' "$1" || true ;;
    *) grep -cE '^\s*(async )?def ' "$1" || true ;;
    esac
}

measured_functions() {
    jq --arg file "$2" '[.[] | select(.file == $file)] | length' "$1"
}

# codemetrics silently skips functions it cannot parse, so unmeasured code must fail.
check_file_coverage() {
    local declared measured
    declared="$(declared_functions "$2")" measured="$(measured_functions "$1" "$2")"
    [[ "$declared" == "$measured" ]] && return 0
    printf 'UNMEASURED %s: %s functions declared, %s measured\n' "$2" "$declared" "$measured"
    return 1
}

check_coverage() {
    local report=$1 file status=0
    shift
    for file in "$@"; do check_file_coverage "$report" "$file" || status=1; done
    return "$status"
}

measured_sources() {
    find core composer scripts tests -type d \( -name target -o -name __pycache__ \) -prune -o \
        -type f \( -name '*.rs' -o -name '*.py' -o -name '*.rb' \) -print
}

grade_functions() {
    local report_dir=$1
    codemetrics --format json core composer scripts tests >"$report_dir/code.json"
    code-inspector -format=json -no-git -no-dup -no-deps -supported-only . >"$report_dir/shell.json"
    jq -s -f scripts/quality-metrics.jq "$report_dir/code.json" "$report_dir/shell.json"
}

report_grades() {
    jq -r '.[] | select(.severity != "OK") | "\(.severity) \(.file):\(.line) \(.function) lines=\(.lines) cyclomatic=\(.cyclomatic) cognitive=\(.cognitive)"' "$1"
    ! jq -e 'any(.[]; .severity == "FAIL")' "$1" >/dev/null
}

audit() {
    local status=0 sources
    report_dir="$(mktemp -d)"
    trap 'rm -rf -- "$report_dir"' EXIT
    grade_functions "$report_dir" >"$report_dir/metrics.json"
    mapfile -t sources < <(measured_sources)
    check_coverage "$report_dir/code.json" "${sources[@]}" || status=1
    report_grades "$report_dir/metrics.json" || status=1
    return "$status"
}

case "${1:-all}" in
all) audit ;;
coverage) check_coverage "${@:2}" ;;
*)
    printf 'Usage: %s [all|coverage REPORT FILE...]\n' "$0" >&2
    exit 2
    ;;
esac
