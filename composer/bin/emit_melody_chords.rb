require_relative "../lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
score.tone(1800.0, 0.010, 0.080, "saw", 3.000)

score.chord(["C3", "E3", "G3"], 0.0, 1.0, 0.25)
score.note("E4", 0.0, 0.5, 0.45)
score.note("G4", 0.5, 0.5, 0.45)

score.chord(["F3", "A3", "C4"], 1.0, 1.0, 0.25)
score.note("A4", 1.0, 0.5, 0.45)
score.note("G4", 1.5, 0.5, 0.45)
