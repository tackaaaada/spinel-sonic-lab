require_relative "../lib/track_score"

score = TrackScore.new(120)
score.sample_file(:kick, "../../samples/kick.wav")

score.track :kick do
    loop bars: 4 do
        use_sample :kick
        bar grid: 4 do
            hits 1, 2, 3, 4
        end
        bar grid: 8 do
            hits 1, 3, 5, 7, 8
        end
        bar grid: 16 do
            hits 1, 5, 9, 13, 16
        end
        bar grid: 64 do
            hits 1, 17, 33, 49, 64
        end
    end
end

score.track :bass do
    loop bars: 4 do
        use_synth :analog_bass
        bar grid: 8 do
            hits 1, 3, note: "C3"
            hits 5, 7, note: "E3"
        end
        bar grid: 16 do
            hits 1, 5, note: "C3"
            hits 9, 13, note: "E3"
        end
        bar grid: 32 do
            hits 1, 9, note: "C3"
            hits 17, 25, note: "G3"
        end
        bar grid: 64 do
            hits 1, 17, note: "C3"
            hits 33, 64, note: "E3"
        end
    end
end

score.emit
