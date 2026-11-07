#!/usr/bin/env bash
# Dispatch this project's source files to its language-specific static analyzers.
set -euo pipefail
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

supported() {
    case "$1" in *.rs | *.rb | *.sh) return 0 ;; *) return 1 ;; esac
}

source_files() {
    local -a newer=()
    [[ $# -eq 0 ]] || newer=(-newer "$1")
    find "$ROOT" \( -type d \( -name .git -o -name .lake -o -name .venv -o -name .astro -o -name node_modules -o -name target -o -name build -o -name dist -o -name generated -o -name sdat-evaluation-generated -o -name __pycache__ \) -prune \) -o -type f "${newer[@]}" -print0
}

check_inside() {
    local rel=$1 file=$2
    [[ "$rel" != ../* && "$rel" != .. ]] || {
        printf 'Outside project: %s\n' "$file" >&2
        return 2
    }
}

relative_path() {
    local file=$1 rel
    [[ -f "$file" ]] || return 0
    rel="$(realpath --relative-to="$ROOT" -- "$file")"
    check_inside "$rel" "$file" || return 2
    if [[ "$rel" != *".generated."* ]] && supported "$rel"; then printf '%s\n' "$rel"; fi
}

normalize_files() {
    local -n output=$1
    shift
    local file rel
    for file in "$@"; do
        rel="$(relative_path "$file")" || return 2
        [[ -z "$rel" ]] || output+=("$rel")
    done
    return 0
}

check_rust() {
    local status=0
    local -a files=()
    collect_files files rs "$@"
    if (("${#files[@]}")); then
        check_rustfmt "${files[@]}" || status=1
        cargo clippy --offline --manifest-path core/Cargo.toml --all-targets || status=1
    fi
    return "$status"
}

check_rustfmt() {
    local file status=0
    for file in "$@"; do
        rustfmt --check --edition 2024 --config skip_children=true "$file" || status=1
    done
    return "$status"
}

# Indentation plus the function limits from docs/quality-policy.md (.rubocop.yml).
RUBOCOP_COPS=Layout/IndentationWidth,Layout/IndentationStyle,Layout/IndentationConsistency,Metrics/MethodLength,Metrics/CyclomaticComplexity

check_ruby() {
    local file status=0
    local -a files=()
    collect_files files rb "$@"
    for file in "${files[@]}"; do ruby -c "$file" || status=1; done
    if (("${#files[@]}")); then rubocop --cache false --only "$RUBOCOP_COPS" "${files[@]}" || status=1; fi
    return "$status"
}

check_shell() {
    local file status=0
    local -a files=()
    collect_files files sh "$@"
    if (("${#files[@]}")); then
        shellcheck "${files[@]}" || status=1
        shfmt -i 4 -d "${files[@]}" || status=1
    fi
    return "$status"
}

# Function length and complexity are scored across the whole project because
# code moves between files when functions are split.
check_function_quality() {
    (($#)) || return 0
    bash scripts/check-quality.sh
}

collect_files() {
    local -n selected=$1
    local extension=$2 file
    shift 2
    for file in "$@"; do
        [[ "$file" == *".$extension" ]] && selected+=("$file")
    done
    return 0
}

check_files() {
    local status=0 check
    local -a files=()
    normalize_files files "$@" || return
    for check in check_rust check_ruby check_shell check_function_quality; do
        "$check" "${files[@]}" || status=1
    done
    return "$status"
}

case "${1:-all}" in
all)
    mapfile -d '' -t paths < <(source_files)
    check_files "${paths[@]}"
    ;;
files)
    shift
    check_files "$@"
    ;;
session | stop)
    mode="$1"
    event="$(cat)"
    session_id="$(jq -r '.session_id // empty' <<<"$event")"
    [[ -n "$session_id" ]] || {
        printf '{}\n'
        exit 0
    }
    state_dir="${TMPDIR:-/tmp}/codex-project-static-check"
    mkdir -p -- "$state_dir"
    marker="$state_dir/$(printf '%s' "$ROOT:$session_id" | sha256sum | cut -d' ' -f1)"
    if [[ "$mode" == session ]]; then
        [[ -e "$marker" ]] || touch -- "$marker"
        printf '{}\n'
        exit 0
    fi
    if [[ -e "$marker" ]]; then
        mapfile -d '' -t paths < <(source_files "$marker")
    else
        mapfile -d '' -t paths < <(source_files)
    fi
    log="$(mktemp)"
    trap 'rm -- "$log"' EXIT
    if check_files "${paths[@]}" >"$log" 2>&1; then
        printf '{}\n'
    elif [[ "$(jq -r '.stop_hook_active // false' <<<"$event")" == true ]]; then
        jq -n --arg message "Static checks still fail: $(head -c 3000 "$log")" '{systemMessage:$message}'
    else
        jq -n --arg reason "Static checks failed: $(head -c 3000 "$log")" '{decision:"block",reason:$reason}'
    fi
    ;;
*)
    printf 'Usage: %s [all|files PATH...|session|stop]\n' "$0" >&2
    exit 2
    ;;
esac
