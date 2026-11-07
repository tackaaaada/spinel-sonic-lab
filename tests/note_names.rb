require_relative "../composer/lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
if ARGV[0] == "chord"
    score.chord(["C3", ARGV[1] || "E3", "G3"], 0.0, 1.0, 0.3)
elsif ARGV[0] == "arpeggio"
    score.track_arpeggio("lead", ["C4", ARGV[1] || "E4", "G4"], 0.0, 0.25, 0.20, 0.5)
elsif ARGV[0] == "bad_arpeggio_step"
    score.arpeggio(["C4", "E4"], 0.0, 0.0, 0.20, 0.5)
else
    score.note(ARGV[0] || "C#4", 0.0, 0.5, 0.8)
end
