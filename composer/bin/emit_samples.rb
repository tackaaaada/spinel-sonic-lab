require_relative "../lib/score_writer"
score = ScoreWriter.new
score.tempo(120)
score.sample_file("kick", "../../samples/kick.wav")
score.sample_file("snare", "../../samples/snare.wav")
score.sample_file("hat", "../../samples/hat.wav")
# Keep the kick on the beat; place a short, defined bass on the offbeat.
score.track_tone_adsr("bass", 1800.0, 0.003, 0.040, 0.45, 0.060, "saw", 3.4)
score.sample_steps("kick", "kick", "x---x---x---x---", 0.0, 0.65)
score.sample_steps("snare", "snare", "----x-------x---", 0.0, 0.65)
score.sample_steps("hat", "hat", "--x---x---x---x-", 0.0, 0.55)
score.track_note("bass", "C2", 0.5, 0.4, 0.75)
score.track_note("bass", "E2", 1.5, 0.4, 0.75)
score.track_note("bass", "G2", 2.5, 0.4, 0.75)
score.track_note("bass", "C3", 3.5, 0.4, 0.75)
