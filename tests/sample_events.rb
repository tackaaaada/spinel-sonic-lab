require_relative "../composer/lib/score_writer"
score = ScoreWriter.new
score.tempo(120)
mode = ARGV[0]
if mode == "bad_name"
    score.sample_file("bad/name", "samples/kick.wav")
elsif mode == "bad_path"
    score.sample_file("kick", "")
elsif mode == "bad_pattern"
    score.sample_file("kick", "samples/kick.wav")
    score.sample_steps("drums", "kick", "x--------------?", 0.0, 0.5)
else
    score.sample_file("kick", "a path/quote\"and\\slash.wav")
    score.sample("kick", 0.0, 0.5)
end
