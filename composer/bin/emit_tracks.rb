require_relative "../lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
score.tone(1400.0, 0.005, 0.040, "saw", 2.800)

score.track_steps("bass", "C2", "x---x---x---x---", 0.0, 0.25, 0.20, 0.55)
score.track_steps("counter", "E2", "--x-------x-----", 0.0, 0.25, 0.20, 0.45)
score.track_steps("counter", "G2", "-------x-------x", 0.0, 0.25, 0.20, 0.45)
