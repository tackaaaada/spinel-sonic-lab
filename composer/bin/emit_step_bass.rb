require_relative "../lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
score.tone(1400.0, 0.005, 0.040, "saw", 2.800)

# Sixteen slots per bar. A hit at slot i begins at i * 0.25 beats.
score.steps("C2", "x---x---x---x---", 0.0, 0.25, 0.20, 0.55)
score.steps("E2", "--x-------x-----", 0.0, 0.25, 0.20, 0.45)
score.steps("G2", "-------x-------x", 0.0, 0.25, 0.20, 0.45)
