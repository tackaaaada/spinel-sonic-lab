require_relative "../composer/lib/track_score"

score = TrackScore.new(120)
score.track :bass do
    loop bars: 4 do
        use_synth :analog_bass
        bar grid: 16 do
            hits 1, 5, note: "C3"
            hits 9, 13, note: "E3"
        end
    end
end
score.emit
