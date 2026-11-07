#!/usr/bin/env bash
# The project's Git hooks must also run Git LFS: a push uploads LFS objects
# only after the static check passes, and LFS finds its hooks in place
# instead of writing untracked copies into .githooks.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() {
    printf 'check_git_hooks: %s\n' "$1" >&2
    exit 1
}

# A repository with the project's hooks, a stub static check that follows
# STATIC_CHECK_STATUS, and one committed LFS sample.
make_repo() {
    git init -q --bare "$work/remote.git"
    git init -q -b main "$work/repo"
    write_files "$work/repo"
    configure_repo
    commit_all
}

write_files() {
    cp -R "$ROOT/.githooks" "$1/.githooks"
    mkdir "$1/scripts" "$1/samples"
    # shellcheck disable=SC2016 # The stub reads the variable when it runs.
    printf 'exit "${STATIC_CHECK_STATUS:-0}"\n' >"$1/scripts/static-check.sh"
    head -c 4096 /dev/urandom >"$1/samples/hit.wav"
}

configure_repo() {
    cd "$work/repo"
    git config core.hooksPath .githooks
    git config user.name "Hook Test"
    git config user.email "hook-test@example.invalid"
    git lfs install --local --skip-repo >/dev/null
    git lfs track '*.wav' >/dev/null
    git remote add origin "file://$work/remote.git"
}

commit_all() {
    git add -A
    git commit -q -m "Add an LFS sample"
}

# Git LFS writes its own hooks into core.hooksPath whenever pre-push is
# missing there; the project's pre-push must keep that from happening.
check_no_stray_hooks() {
    head -c 4096 /dev/urandom >samples/hit.wav
    git stash -q
    git stash drop -q
    # Compare listings, not git status: `git add -A` would commit stray hooks.
    [ "$(ls .githooks)" = "$(ls "$ROOT/.githooks")" ] || fail "Git LFS wrote its own hooks into .githooks"
}

lfs_objects_on_remote() {
    find "$work/remote.git/lfs/objects" -type f 2>/dev/null | wc -l
}

check_failed_static_check_blocks_upload() {
    if STATIC_CHECK_STATUS=1 git push -q origin main 2>/dev/null; then
        fail "push succeeded although the static check failed"
    fi
    [ "$(lfs_objects_on_remote)" -eq 0 ] || fail "LFS objects were uploaded before the static check passed"
}

check_push_uploads_lfs_objects() {
    git push -q origin main 2>/dev/null || fail "push failed with a passing static check"
    [ "$(lfs_objects_on_remote)" -eq 1 ] || fail "the pushed LFS sample is missing on the remote"
}

make_repo
check_no_stray_hooks
check_failed_static_check_blocks_upload
check_push_uploads_lfs_objects
printf 'Git hooks run the static check and Git LFS\n'
