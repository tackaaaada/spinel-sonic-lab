require_relative "../lib/track_score"

# Each track reads top-down like a console channel module:
# source -> H/A -> Dynamics -> EQ -> Aux -> Pan -> Fader, then the Grid loop.
score = TrackScore.new(124)
score.sample_file(:kick, "../../samples/kick.wav")
score.sample_file(:hat, "../../samples/hat.wav")

score.aux_return :room do
    reverb room: 0.6, damp: 0.4, wet: 0.5
    fader db: -6
end

score.track :kick do
    use_sample :kick
    ha gain_db: 0
    dynamics do
        compressor threshold_db: -18, ratio: 3, attack_ms: 10, release_ms: 100
    end
    eq do
        bell frequency_hz: 80, gain_db: 2, q: 1
    end
    pan position: 0
    fader db: -6
    loop bars: 4 do
        bar grid: 16 do
            hits 1, 5, 9, 13, velocity: 0.7
        end
    end
end

score.track :hat do
    use_sample :hat
    ha gain_db: -3
    eq do
        bell frequency_hz: 8000, gain_db: -3, q: 0.7
    end
    aux do
        send_to :room, level_db: -18
    end
    pan position: 0.4
    fader db: -12
    loop bars: 4 do
        bar grid: 16 do
            hits 3, 7, 11, 15, velocity: 0.5
        end
    end
end

score.track :bass do
    use_synth :analog_bass
    ha gain_db: 0
    dynamics do
        compressor threshold_db: -16, ratio: 2, attack_ms: 15, release_ms: 120
    end
    eq do
        bell frequency_hz: 180, gain_db: -2, q: 1
    end
    aux do
        send_to :room, level_db: -24
    end
    pan position: -0.2
    fader db: -6
    loop bars: 4 do
        bar grid: 16 do
            hits 1, 4, note: "C2", duration_beats: 0.5, velocity: 0.7
            hits 9, note: "E2", duration_beats: 1.0, velocity: 0.8
        end
        bar grid: 16 do
            hits 1, 7, note: "G1", duration_beats: 0.5, velocity: 0.7
            hits 11, note: "A#1", duration_beats: 1.0, velocity: 0.75
        end
    end
end

score.emit
