require_relative "../composer/lib/score_writer"

score = ScoreWriter.new
score.tempo(120)
case ARGV[0]
when "tone"
    score.tone(1200.0, 0.01, 0.05, "triangle")
when "adsr"
    score.tone_adsr(1200.0, 0.01, 0.08, 0.7, 0.05, "triangle")
when "lfo"
    score.tone_lfo(1200.0, 0.01, 0.08, 0.7, 0.05, 4.0, 0.5, "triangle")
end
