require_relative "../lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
score.tone_lfo(1200.0, 0.005, 0.040, 0.800, 0.060, 4.000, 0.900, "saw", 3.500)
score.note("C2", 0.0, 0.5, 0.8)
score.note("E2", 0.5, 0.5, 0.8)
score.note("G2", 1.0, 0.5, 0.8)
score.note("C3", 1.5, 0.5, 0.8)
