require_relative "../composer/lib/track_score"

score = TrackScore.new(120)
score.sample_file(:kick, "../../samples/kick.wav")
score.aux_return :room do
    reverb room: 0.35, damp: 0.45, wet: 0.25
    fader db: -12
end

score.track :kick do
    use_sample :kick
    ha gain_db: 0
    if ARGV[0] == "bad_order"
        eq { bell frequency_hz: 80, gain_db: 2, q: 1 }
        dynamics { compressor threshold_db: -18, ratio: 3, attack_ms: 10, release_ms: 100 }
    else
        dynamics { compressor threshold_db: -18, ratio: 3, attack_ms: 10, release_ms: 100 }
        eq { bell frequency_hz: 80, gain_db: 2, q: 1 }
    end
    aux do
        if ARGV[0] == "missing_return"
            send_to :missing, level_db: -18
        else
            send_to :room, level_db: -18
        end
    end
    pan position: 0
    fader db: -6
    loop bars: 4 do
        bar grid: 16 do
            hits 1, 5, 9, 13, velocity: 0.65
        end
    end
end

score.emit
