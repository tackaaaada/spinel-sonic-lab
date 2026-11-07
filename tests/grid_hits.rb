require_relative "../composer/lib/track_score"

grid = ARGV[0].to_i
score = TrackScore.new(120)
score.sample_file(:kick, "../samples/kick.wav")
score.track :kick do
    loop bars: 4 do
        use_sample :kick
        if ARGV[0] == "mixed"
            bar grid: 4 do
                hits 1, 4
            end
            bar grid: 8 do
                hits 1, 8
            end
            bar grid: 32 do
                hits 1, 32
            end
            bar grid: 64 do
                hits 1, 64
            end
        elsif ARGV[0] == "bad_hit"
            bar grid: 16 do
                hits 1, 17
            end
        elsif ARGV[0] == "bad_fraction"
            bar grid: 16 do
                hits 1.5
            end
        else
            bar grid: grid do
                hits 1, grid
            end
        end
    end
end
score.emit
