require_relative "../composer/lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
if ARGV[0] == "preset"
    score.track_preset(ARGV[1], ARGV[2] || "soft")
    score.track_note(ARGV[1], "C2", 0.0, 0.5, 0.7)
    score.track_chord("chords", ["C3", "E3", "G3"], 0.0, 0.5, 0.35)
elsif ARGV[0] == "drums"
    score.track_preset("kick", "kick")
    score.track_preset("snare", "snare")
    score.track_preset("hat", "hat")
    score.drum_steps(ARGV[1] || "kick", "x---x---x---x---", 0.0, 0.8)
    score.drum_steps("snare", "----x-------x---", 0.0, 0.75)
    score.drum_steps("hat", "--x---x---x---x-", 0.0, 0.55)
else
    score.track_steps(ARGV[0], "C2", "x---x---x---x---", 0.0, 0.25, 0.20, 0.5)
end

