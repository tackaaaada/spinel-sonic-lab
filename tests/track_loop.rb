require_relative "../composer/lib/track_score"

score = TrackScore.new(120)
score.sample_file(:bd_303, "../samples/kick.wav")
mode = ARGV[0]
score.track :kick do
    loop bars: mode == "bad_bars" ? 8 : 4 do
        use_sample :bd_303
        if mode == "bad_hit"
            bar { hits 1, 4.25 }
        elsif mode == "bad_grid"
            bar { hits 1, 3.7 }
        elsif mode == "straight"
            bar { hits 1, 2, 3, 4 }
        else
            bar { hits 1, 2, 3, 3.75 }
        end
    end
    bar { hits 1 } if mode == "bar_outside"
end
score.emit
