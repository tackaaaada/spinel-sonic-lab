require_relative "../composer/lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
if ARGV[0] == "bad_length"
    score.steps("C2", "x---", 0.0, 0.25, 0.20, 0.5)
elsif ARGV[0] == "bad_symbol"
    score.steps("C2", "x---x---x---x--?", 0.0, 0.25, 0.20, 0.5)
elsif ARGV[0] == "bad_note"
    score.steps("H2", "x---x---x---x---", 0.0, 0.25, 0.20, 0.5)
elsif ARGV[0] == "bad_grid"
    score.steps("C2", "x---x---x---x---", 0.0, 0.0, 0.20, 0.5)
elsif ARGV[0] == "bad_bars"
    score.loop_steps("C2", "x---x---x---x---", 0, 0.25, 0.20, 0.5)
elsif ARGV[0] == "loop"
    score.track_loop_steps("bass", "C2", "x-------x-------", 2, 0.25, 0.20, 0.5)
    score.drum_loop_steps("kick", "x---x---x---x---", 2, 0.8)
else
    score.steps("C2", "x---x---x---x---", 0.0, 0.25, 0.20, 0.5)
end
