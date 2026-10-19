# A deliberately small Ruby subset that Spinel can compile ahead of time.
class ScoreWriter
  def tempo(bpm)
    printf("{\"type\":\"tempo\",\"bpm\":%d}\n", bpm)
  end

  # Declare assets before all playback events. Rust resolves paths relative to the score.
  def sample_file(name, path)
    validate_track_name(name)
    unless path.is_a?(String) && path.length > 0 && path.length <= 4096
      raise "invalid sample path"
    end
    escaped = ""
    index = 0
    while index < path.length
      char = path[index]
      if char < " "
        raise "sample path contains a control character"
      end
      if char == "\\" || char == "\""
        escaped = escaped + "\\"
      end
      escaped = escaped + char
      index += 1
    end
    printf("{\"type\":\"sample\",\"name\":\"%s\",\"path\":\"%s\"}\n", name, escaped)
  end

  def sample(name, beat, velocity = 1.0)
    track_sample("main", name, beat, velocity)
  end

  def track_sample(track, name, beat, velocity = 1.0)
    validate_track_name(track)
    validate_track_name(name)
    unless beat >= 0 && velocity >= 0 && velocity <= 1
      raise "invalid sample beat or velocity"
    end
    printf("{\"type\":\"sample_note\",\"sample\":\"%s\",\"track\":\"%s\",\"beat\":%.3f,\"velocity\":%.3f}\n", name, track, beat, velocity)
  end

  def sample_steps(track, name, pattern, start_beat, velocity = 1.0)
    validate_track_name(track)
    validate_track_name(name)
    validate_step_pattern(pattern, 0.25)
    unless start_beat >= 0 && velocity >= 0 && velocity <= 1
      raise "invalid sample beat or velocity"
    end
    index = 0
    while index < 16
      if pattern[index] == "x"
        track_sample(track, name, start_beat + index * 0.25, velocity)
      end
      index += 1
    end
  end

  def tone(cutoff_hz, attack_seconds, release_seconds, oscillator = "saw", gain = 1.0)
    write_tone(nil, cutoff_hz, attack_seconds, release_seconds, oscillator, gain)
  end

  def track_tone(track, cutoff_hz, attack_seconds, release_seconds, oscillator = "saw", gain = 1.0)
    validate_track_name(track)
    write_tone(track, cutoff_hz, attack_seconds, release_seconds, oscillator, gain)
  end

  def tone_adsr(cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, oscillator = "saw", gain = 1.0)
    write_tone_adsr(nil, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, oscillator, gain)
  end

  def track_tone_adsr(track, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, oscillator = "saw", gain = 1.0)
    validate_track_name(track)
    write_tone_adsr(track, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, oscillator, gain)
  end

  def tone_lfo(cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, lfo_rate_hz, lfo_depth, oscillator = "saw", gain = 1.0)
    write_tone_lfo(nil, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, lfo_rate_hz, lfo_depth, oscillator, gain)
  end

  def track_tone_lfo(track, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, lfo_rate_hz, lfo_depth, oscillator = "saw", gain = 1.0)
    validate_track_name(track)
    write_tone_lfo(track, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, lfo_rate_hz, lfo_depth, oscillator, gain)
  end

  def preset(name)
    write_preset(nil, name)
  end

  def track_preset(track, name)
    validate_track_name(track)
    write_preset(track, name)
  end

  def write_preset(track, name)
    case name
    when "soft"
      write_tone_adsr(track, 1200.0, 0.020, 0.080, 0.650, 0.100, "sine", 1.600)
    when "bright"
      write_tone(track, 1800.0, 0.005, 0.060, "saw", 2.600)
    when "wobble"
      write_tone_lfo(track, 1200.0, 0.005, 0.040, 0.800, 0.060, 4.000, 0.900, "saw", 3.500)
    when "kick"
      write_tone_adsr(track, 320.0, 0.001, 0.080, 0.000, 0.090, "sine", 3.800)
    when "snare"
      write_tone_adsr(track, 5500.0, 0.001, 0.070, 0.000, 0.070, "noise", 3.400)
    when "hat"
      write_tone_adsr(track, 14000.0, 0.001, 0.030, 0.000, 0.030, "noise", 2.900)
    else
      raise "unknown preset: #{name}"
    end
  end

  def write_tone(track, cutoff_hz, attack_seconds, release_seconds, oscillator, gain)
    validate_oscillator(oscillator)
    if track
      printf("{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"%s\",\"oscillator\":\"%s\",\"cutoff_hz\":%.1f,\"attack_seconds\":%.3f,\"release_seconds\":%.3f,\"gain\":%.3f}\n",
             track, oscillator, cutoff_hz, attack_seconds, release_seconds, gain)
    else
      printf("{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"%s\",\"cutoff_hz\":%.1f,\"attack_seconds\":%.3f,\"release_seconds\":%.3f,\"gain\":%.3f}\n",
             oscillator, cutoff_hz, attack_seconds, release_seconds, gain)
    end
  end

  def write_tone_adsr(track, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, oscillator, gain)
    validate_oscillator(oscillator)
    if track
      printf("{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"%s\",\"oscillator\":\"%s\",\"cutoff_hz\":%.1f,\"attack_seconds\":%.3f,\"decay_seconds\":%.3f,\"sustain_level\":%.3f,\"release_seconds\":%.3f,\"gain\":%.3f}\n",
             track, oscillator, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, gain)
    else
      printf("{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"%s\",\"cutoff_hz\":%.1f,\"attack_seconds\":%.3f,\"decay_seconds\":%.3f,\"sustain_level\":%.3f,\"release_seconds\":%.3f,\"gain\":%.3f}\n",
             oscillator, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, gain)
    end
  end

  def write_tone_lfo(track, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, lfo_rate_hz, lfo_depth, oscillator, gain)
    validate_oscillator(oscillator)
    if track
      printf("{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"track\":\"%s\",\"oscillator\":\"%s\",\"cutoff_hz\":%.1f,\"attack_seconds\":%.3f,\"decay_seconds\":%.3f,\"sustain_level\":%.3f,\"release_seconds\":%.3f,\"lfo_rate_hz\":%.3f,\"lfo_depth\":%.3f,\"gain\":%.3f}\n",
             track, oscillator, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, lfo_rate_hz, lfo_depth, gain)
    else
      printf("{\"type\":\"tone\",\"instrument\":\"analog_bass\",\"oscillator\":\"%s\",\"cutoff_hz\":%.1f,\"attack_seconds\":%.3f,\"decay_seconds\":%.3f,\"sustain_level\":%.3f,\"release_seconds\":%.3f,\"lfo_rate_hz\":%.3f,\"lfo_depth\":%.3f,\"gain\":%.3f}\n",
             oscillator, cutoff_hz, attack_seconds, decay_seconds, sustain_level, release_seconds, lfo_rate_hz, lfo_depth, gain)
    end
  end

  def note(name, beat, duration_beats, velocity)
    midi_note(midi_for_name(name), beat, duration_beats, velocity)
  end

  def track_note(track, name, beat, duration_beats, velocity)
    validate_track_name(track)
    midi_note(midi_for_name(name), beat, duration_beats, velocity, track)
  end

  def chord(names, beat, duration_beats, velocity)
    write_chord(nil, names, beat, duration_beats, velocity)
  end

  def track_chord(track, names, beat, duration_beats, velocity)
    validate_track_name(track)
    write_chord(track, names, beat, duration_beats, velocity)
  end

  def write_chord(track, names, beat, duration_beats, velocity)
    notes = midi_list_for_names(names, "chord needs at least two notes")
    notes.each do |midi|
      midi_note(midi, beat, duration_beats, velocity, track)
    end
  end

  def arpeggio(names, start_beat, step_beats, duration_beats, velocity)
    write_arpeggio(nil, names, start_beat, step_beats, duration_beats, velocity)
  end

  def track_arpeggio(track, names, start_beat, step_beats, duration_beats, velocity)
    validate_track_name(track)
    write_arpeggio(track, names, start_beat, step_beats, duration_beats, velocity)
  end

  def write_arpeggio(track, names, start_beat, step_beats, duration_beats, velocity)
    notes = midi_list_for_names(names, "arpeggio needs at least two notes")
    unless step_beats > 0
      raise "step length must be positive"
    end
    index = 0
    while index < notes.length
      midi_note(notes[index], start_beat + index * step_beats, duration_beats, velocity, track)
      index += 1
    end
  end

  def midi_list_for_names(names, empty_error)
    notes = []
    names.each do |name|
      notes << midi_for_name(name)
    end
    if notes.length < 2
      raise empty_error
    end
    notes
  end

  # Sixteen slots: x plays, - rests. A 0.25-beat slot makes one 4/4 bar.
  def steps(name, pattern, start_beat, step_beats, duration_beats, velocity)
    write_steps(nil, name, pattern, start_beat, step_beats, duration_beats, velocity)
  end

  def track_steps(track, name, pattern, start_beat, step_beats, duration_beats, velocity)
    validate_track_name(track)
    write_steps(track, name, pattern, start_beat, step_beats, duration_beats, velocity)
  end

  def loop_steps(name, pattern, bars, step_beats, duration_beats, velocity)
    write_loop_steps(nil, name, pattern, bars, step_beats, duration_beats, velocity)
  end

  def track_loop_steps(track, name, pattern, bars, step_beats, duration_beats, velocity)
    validate_track_name(track)
    write_loop_steps(track, name, pattern, bars, step_beats, duration_beats, velocity)
  end

  def drum_steps(drum, pattern, start_beat, velocity = 0.75)
    pitch = drum_pitch(drum)
    duration_beats = drum_duration(drum)
    write_steps(drum, pitch, pattern, start_beat, 0.25, duration_beats, velocity)
  end

  def drum_loop_steps(drum, pattern, bars, velocity = 0.75)
    pitch = drum_pitch(drum)
    duration_beats = drum_duration(drum)
    write_loop_steps(drum, pitch, pattern, bars, 0.25, duration_beats, velocity)
  end

  def drum_pitch(drum)
    case drum
    when "kick" then "C2"
    when "snare" then "D3"
    when "hat" then "F#4"
    else
      raise "unknown drum: #{drum}"
    end
  end

  def drum_duration(drum)
    case drum
    when "kick" then 0.36
    when "snare" then 0.28
    when "hat" then 0.14
    else
      raise "unknown drum: #{drum}"
    end
  end

  def write_steps(track, name, pattern, start_beat, step_beats, duration_beats, velocity)
    midi = midi_for_name(name)
    validate_step_pattern(pattern, step_beats)
    emit_step_pattern(track, midi, pattern, start_beat, step_beats, duration_beats, velocity)
  end

  def write_loop_steps(track, name, pattern, bars, step_beats, duration_beats, velocity)
    midi = midi_for_name(name)
    unless bars.is_a?(Integer) && bars >= 1
      raise "bars must be a positive integer"
    end
    validate_step_pattern(pattern, step_beats)
    bar_beats = 16 * step_beats
    bar = 0
    while bar < bars
      emit_step_pattern(track, midi, pattern, bar * bar_beats, step_beats, duration_beats, velocity)
      bar += 1
    end
  end

  def validate_step_pattern(pattern, step_beats)
    unless pattern.length == 16
      raise "step pattern must have 16 characters"
    end
    unless step_beats > 0
      raise "step length must be positive"
    end
    index = 0
    while index < 16
      unless pattern[index] == "x" || pattern[index] == "-"
        raise "step pattern accepts only x and -"
      end
      index += 1
    end
  end

  def emit_step_pattern(track, midi, pattern, start_beat, step_beats, duration_beats, velocity)
    index = 0
    while index < 16
      if pattern[index] == "x"
        midi_note(midi, start_beat + index * step_beats, duration_beats, velocity, track)
      end
      index += 1
    end
  end

  def midi_note(midi, beat, duration_beats, velocity, track = nil)
    unless midi.is_a?(Integer) && midi >= 0 && midi <= 127
      raise "MIDI note must be an integer from 0 to 127"
    end
    if track
      validate_track_name(track)
      printf("{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"%s\",\"note\":%d,\"beat\":%.3f,\"duration_beats\":%.3f,\"velocity\":%.3f}\n",
             track, midi, beat, duration_beats, velocity)
    else
      printf("{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":%d,\"beat\":%.3f,\"duration_beats\":%.3f,\"velocity\":%.3f}\n",
             midi, beat, duration_beats, velocity)
    end
  end

  def midi_for_name(name)
    unless name.length == 2 || name.length == 3
      raise "unknown note: #{name}"
    end
    semitone = case name[0]
               when "C" then 0
               when "D" then 2
               when "E" then 4
               when "F" then 5
               when "G" then 7
               when "A" then 9
               when "B" then 11
               else
                 raise "unknown note: #{name}"
               end
    octave_digit = name[1]
    if name.length == 3
      unless name[1] == "#"
        raise "unknown note: #{name}"
      end
      semitone += 1
      octave_digit = name[2]
    end
    unless octave_digit >= "0" && octave_digit <= "9"
      raise "unknown note: #{name}"
    end
    midi = (octave_digit.to_i + 1) * 12 + semitone
    unless midi <= 127
      raise "note is outside MIDI range: #{name}"
    end
    midi
  end

  def validate_oscillator(oscillator)
    unless oscillator == "saw" || oscillator == "sine" || oscillator == "pulse" || oscillator == "noise"
      raise "unknown oscillator: #{oscillator}"
    end
  end

  def validate_track_name(track)
    unless track.is_a?(String) && track.length >= 1 && track.length <= 32
      raise "invalid track name"
    end
    index = 0
    while index < track.length
      char = track[index]
      unless (char >= "A" && char <= "Z") || (char >= "a" && char <= "z") ||
             (char >= "0" && char <= "9") || char == "_" || char == "-"
        raise "invalid track name"
      end
      index += 1
    end
  end
end

