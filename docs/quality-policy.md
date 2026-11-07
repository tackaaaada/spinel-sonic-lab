# Source quality checks

The existing `mise run static-check` task checks Rust, Ruby, and Shell with
their language-specific analyzers. Rust uses rustfmt and Clippy; Ruby uses
`ruby -c` and RuboCop; Shell uses ShellCheck and shfmt. Indentation is four
spaces in all three languages. Ruby's RuboCop and
Shell's shfmt checks require `mise install` to provide their pinned tools.

## Function limits

The policy counts physical lines from the function declaration through its
end. More than 10 lines is a failure. Cyclomatic complexity 1–10 passes,
11–20 warns, and 21 or more fails. Cognitive complexity 0–4 passes, 5–14
warns, and 15 or more fails. A warning calls for a reviewed refactor. Both
metrics are per function, not file totals.

`static-check` enforces these limits through `bash scripts/check-quality.sh`,
which `mise run quality-audit` also runs after its grade-boundary tests. The
script scores the whole project's working tree, so the pre-commit hook can also
report unstaged files. Since 2026-11-07 every function passes with no warnings.

The script uses the pinned `codemetrics` CLI for Rust, Python, and Ruby. The CLI
follows SonarSource's cognitive-complexity model and matches `gocognit` on Go
examples. The original `gocognit` program accepts Go source only, so it cannot
parse this project's four languages. Shell uses `code-inspector`'s Bash parser;
its complexity scores are estimates, so they warn but never fail. Shell
function length is still enforced.

`codemetrics` silently skips functions that its parser cannot read. It
skipped 14 of 19 functions in `schedule.rs` before that file was refactored.
`check-quality.sh` therefore compares each Rust, Python, and Ruby file's
declared functions with the measured ones and fails with `UNMEASURED` on a
mismatch. When that happens, rewrite the reported code so it parses. For
example, moving a function above the one that confuses the parser fixed
`model.rs`.

## Native analyzer limits

Each language's own analyzer also carries the limits it can express. These
checks back up the parser above and report problems inside the editor:

| Language | Setting | Limit |
| --- | --- | --- |
| Rust | `clippy::too_many_lines` (denied in `core/Cargo.toml`, threshold in `core/clippy.toml`) | 10 code lines; Clippy skips blank and comment lines |
| Ruby | RuboCop `Metrics/MethodLength` and `Metrics/CyclomaticComplexity` (`.rubocop.yml`) | 8 body lines (10 with `def`/`end`); complexity 20 |
| Shell | none available | covered by `check-quality.sh` |

Clippy's `cognitive_complexity` lint is not used. Its own documentation says it
does not measure cognitive complexity reliably. RuboCop's
`Metrics/PerceivedComplexity` uses a different model, so cognitive complexity
comes only from `codemetrics`.

Rust formatting uses `use_small_heuristics = "Max"` (`core/rustfmt.toml`). It
keeps a statement on one line up to the 100-column limit, so the line limit
measures logic rather than wrapping.

Run `bash tests/check_quality_metrics.sh` to check the grade boundaries and the
coverage check independently of the source audit.
