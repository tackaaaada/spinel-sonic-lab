require_relative "../composer/lib/track_score"

score = TrackScore.new(120)
score.sample_file(:kick, "../../samples/kick.wav")

score.track :kick do
    use_sample :kick
    ha gain_db: 6
    if ARGV[0] == "bad_order"
        fader db: -6
        pan position: -1
    elsif ARGV[0] == "bad_pan"
        pan position: -1.1
    else
        pan position: -1
        fader db: -6
    end
    loop bars: 4 do
        bar grid: 16 do
            hits 1, 5, velocity: 0.4
        end
    end
end

score.track :bass do
    use_synth :analog_bass
    ha gain_db: 0
    pan position: 1
    fader db: -6
    loop bars: 4 do
        bar grid: 16 do
            hits 1, note: "C3", duration_beats: 0.5, velocity: 0.5
        end
    end
end

score.emit
