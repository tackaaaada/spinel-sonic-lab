require_relative "../lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
score.preset(ARGV[0] || "soft")
score.note("C2", 0.0, 0.5, 0.8)
score.note("E2", 0.5, 0.5, 0.8)
score.note("G2", 1.0, 0.5, 0.8)
score.note("C3", 1.5, 0.5, 0.8)
