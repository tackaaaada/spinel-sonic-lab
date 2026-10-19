# frozen_string_literal: true

require_relative '../lib/score_writer'

score = ScoreWriter.new
score.tempo(120)

# 1. トラックごとの音色・帯域設計（6パートが帯域と音量で棲み分ける設定）
score.track_preset('kick', 'kick')
score.track_preset('snare', 'snare')
score.track_preset('hat', 'hat')
# ベース: キック(65Hz付近)の上の中低域(cutoff 1800Hz)を太く鳴らすSaw
score.track_tone_adsr('bass', 1800.0, 0.005, 0.10, 0.75, 0.08, 'saw', 3.4)
# コード: 中域(cutoff 1200Hz)を柔らかく包むPulseパッド
score.track_tone_adsr('chords', 1200.0, 0.02, 0.10, 0.65, 0.12, 'pulse', 1.8)
# メロディ: コードやドラムに埋もれない中高域(cutoff 3800Hz)のリードSaw
score.track_tone_adsr('melody', 3800.0, 0.005, 0.08, 0.75, 0.08, 'saw', 3.4)

# 2. ドラム（16ステップ × 1小節：他パートを潰さず芯を作るベロシティ）
score.drum_steps('kick',  'x---x---x---x---', 0.0, 0.62)
score.drum_steps('snare', '----x-------x---', 0.0, 0.58)
score.drum_steps('hat',   '--x---x---x---x-', 0.0, 0.55)

# 3. コード（0.0拍目と2.0拍目：3和音合計でベース・メロディと同等のRMSになるベロシティ）
score.track_chord('chords', %w[C3 E3 G3], 0.0, 1.5, 0.38)
score.track_chord('chords', %w[F3 A3 C4], 2.0, 1.5, 0.38)

# 4. ベース（裏拍・シンコペーションでキックと噛み合わせる）
score.track_note('bass', 'C2', 0.5, 0.45, 0.75)
score.track_note('bass', 'G2', 1.5, 0.45, 0.75)
score.track_note('bass', 'F2', 2.5, 0.45, 0.75)
score.track_note('bass', 'C3', 3.5, 0.45, 0.75)

# 5. メロディ（アルペジオ補助で8分音符を展開：リードとして前に出るベロシティとゲート長）
score.track_arpeggio('melody', %w[E4 G4 C5 G4], 0.0, 0.5, 0.42, 0.62)
score.track_arpeggio('melody', %w[A4 F4 E4 D4], 2.0, 0.5, 0.42, 0.62)
