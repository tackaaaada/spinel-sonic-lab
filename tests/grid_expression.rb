require_relative "../composer/lib/track_score"

score = TrackScore.new(120)
score.sample_file(:kick, "../../samples/kick.wav")

score.track :kick do
    loop bars: 4 do
        use_sample :kick
        bar grid: 16 do
            if ARGV[0] == "sample_duration"
                hits 1, duration_beats: 0.5
            elsif ARGV[0] == "bad_velocity"
                hits 1, velocity: 1.1
            else
                hits 1, 5, velocity: 0.4
            end
        end
    end
end

score.track :bass do
    loop bars: 4 do
        use_synth :analog_bass
        bar grid: 16 do
            if ARGV[0] == "bad_duration"
                hits 1, note: "C3", duration_beats: 0
            else
                hits 1, 5, note: "C3", duration_beats: 0.5, velocity: 0.7
                hits 9, note: "E3", duration_beats: 1.0, velocity: 0.8
            end
        end
    end
end

score.emit
