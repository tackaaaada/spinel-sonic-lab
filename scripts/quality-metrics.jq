# Shell scores are estimates, so they warn but never fail.
def complexity_grade:
    if .source == "shell-estimate" and (.cyclomatic >= 11 or .cognitive >= 5) then "WARN"
    elif .cyclomatic > 20 or .cognitive >= 15 then "FAIL"
    elif .cyclomatic >= 11 or .cognitive >= 5 then "WARN"
    else "OK" end;

def grade:
    if .lines > 10 then "FAIL" else complexity_grade end;

([.[0][] | {
    file, function,
    line: .start_line,
    lines: (.end_line - .start_line + 1),
    cyclomatic, cognitive,
    source: "codemetrics"
}] + [.[1].root | .. | objects
    | select((.RelPath? // "") | endswith(".sh"))
    | . as $file
    | .Metrics.Functions[]?
    | {
        file: $file.RelPath,
        function: .Name,
        line: .Line,
        lines: .LineCount,
        cyclomatic: .Cyclomatic,
        cognitive: .Cognitive,
        source: "shell-estimate"
    }
]) | map(. + {severity: grade}) | sort_by(.file, .line)
