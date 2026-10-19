require_relative "../lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
score.tone_adsr(1800.0, 0.005, 0.120, 0.250, 0.060, "saw", 3.000)
score.note("C2", 0.0, 0.5, 0.8)
score.note("E2", 0.5, 0.5, 0.8)
score.note("G2", 1.0, 0.5, 0.8)
score.note("C3", 1.5, 0.5, 0.8)
