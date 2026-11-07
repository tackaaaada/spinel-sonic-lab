require_relative "../lib/score_writer"

score = ScoreWriter.new
score.tempo(133)
score.sample_file("kick", "../../samples/kick.wav")
score.sample_file("snare", "../../samples/snare.wav")
score.sample_file("hat", "../../samples/hat.wav")
score.track_tone_lfo("bass", 900.0, 0.003, 0.040, 0.400, 0.060, 2.0, 0.300, "saw", 3.4)

# Two bars: steady four-on-the-floor drums and a short offbeat bass pulse.
score.sample_steps("kick", "kick", "x---x---x---x---", 0.0, 0.65)
score.sample_steps("kick", "kick", "x---x---x---x---", 4.0, 0.65)
score.sample_steps("snare", "snare", "----x-------x---", 0.0, 0.55)
score.sample_steps("snare", "snare", "----x-------x---", 4.0, 0.55)
score.sample_steps("hat", "hat", "--x---x---x---x-", 0.0, 0.42)
score.sample_steps("hat", "hat", "--x---x---x-xx-x", 4.0, 0.42)
score.track_steps("bass", "C2", "--x---x---x---x-", 0.0, 0.25, 0.32, 0.55)
score.track_steps("bass", "C2", "--x---x---x-----", 4.0, 0.25, 0.32, 0.55)
score.track_note("bass", "D#2", 7.5, 0.32, 0.55)
