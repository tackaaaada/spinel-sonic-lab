require_relative "../lib/track_score"

score = TrackScore.new(120)
score.sample_file(:bd_303, "../../samples/kick.wav")
score.track :kick do
    loop bars: 4 do
        use_sample :bd_303

      # bar { hits 1, 2, 3, 4 }
        bar { hits 1, 2, 3, 3.75 }
    end
end
score.emit
