# A deliberately small Ruby subset that Spinel can compile ahead of time.
class ScoreWriter
    def tempo(bpm, bars = nil)
        if bars
            printf("{\"type\":\"tempo\",\"bpm\":%d,\"bars\":%d}\n", bpm, bars)
        else
            printf("{\"type\":\"tempo\",\"bpm\":%d}\n", bpm)
        end
    end

  # Declare assets before all playback events. Rust resolves paths relative to the score.
    def sample_file(name, path)
        validate_track_name(name)
        validate_sample_path(path)
        printf("{\"type\":\"sample\",\"name\":\"%s\",\"path\":\"%s\"}\n", name, escape_sample_path(path))
    end

    def validate_sample_path(path)
        raise "invalid sample path" unless path.is_a?(String) && path.length > 0 && path.length <= 4096
    end

    def escape_sample_path(path)
        escaped = ""
        index = 0
        while index < path.length
            char = path[index]
            escaped = escaped + escape_sample_char(char)
            index += 1
        end
        escaped
    end

    def escape_sample_char(char)
        raise "sample path contains a control character" if char < " "
        return "\\" + char if char == "\\" || char == "\""
        char
    end

    def channel(track, ha_gain_db, pan, fader_db, compressor = nil, bell = nil, aux_send = nil)
        validate_track_name(track)
        printf("{\"type\":\"channel\",\"track\":\"%s\",\"ha_gain_db\":%.3f,\"pan\":%.3f,\"fader_db\":%.3f",
               track, ha_gain_db, pan, fader_db)
        write_channel_controls(compressor, bell, aux_send)
        printf("}\n")
    end

    def write_channel_controls(compressor, bell, aux_send)
        write_compressor(compressor) if compressor
        write_bell(bell) if bell
        write_aux_send(aux_send) if aux_send
    end

    def write_compressor(compressor)
        printf(",\"compressor\":{\"threshold_db\":%.3f,\"ratio\":%.3f,\"attack_ms\":%.3f,\"release_ms\":%.3f}",
               compressor[0], compressor[1], compressor[2], compressor[3])
    end

    def write_bell(bell)
        printf(",\"eq_bell\":{\"frequency_hz\":%.3f,\"gain_db\":%.3f,\"q\":%.3f}",
               bell[0], bell[1], bell[2])
    end

    def write_aux_send(aux_send)
        printf(",\"aux_send\":{\"return\":\"%s\",\"level_db\":%.3f}", aux_send[0], aux_send[1])
    end

    def aux_return(name, reverb, fader_db)
        validate_track_name(name)
        printf("{\"type\":\"aux_return\",\"name\":\"%s\",\"reverb\":{\"room\":%.3f,\"damp\":%.3f,\"wet\":%.3f},\"fader_db\":%.3f}\n",
               name, reverb[0], reverb[1], reverb[2], fader_db)
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
        printf("{\"type\":\"sample_note\",\"sample\":\"%s\",\"track\":\"%s\",\"beat\":%.4f,\"velocity\":%.3f}\n", name, track, beat, velocity)
    end

    def sample_steps(track, name, pattern, start_beat, velocity = 1.0)
        validate_track_name(track)
        validate_track_name(name)
        validate_step_pattern(pattern, 0.25)
        unless start_beat >= 0 && velocity >= 0 && velocity <= 1
            raise "invalid sample beat or velocity"
        end
        emit_sample_steps(track, name, pattern, start_beat, velocity)
    end

    def emit_sample_steps(track, name, pattern, start_beat, velocity)
        index = 0
        while index < 16
            track_sample(track, name, start_beat + index * 0.25, velocity) if pattern[index] == "x"
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
        when "soft" then write_tone_adsr(track, 1200.0, 0.020, 0.080, 0.650, 0.100, "sine", 1.600)
        when "bright" then write_tone(track, 1800.0, 0.005, 0.060, "saw", 2.600)
        when "wobble" then write_tone_lfo(track, 1200.0, 0.005, 0.040, 0.800, 0.060, 4.000, 0.900, "saw", 3.500)
        else write_drum_preset(track, name)
        end
    end

    def write_drum_preset(track, name)
        case name
        when "kick" then write_tone_adsr(track, 320.0, 0.001, 0.080, 0.000, 0.090, "sine", 3.800)
        when "snare" then write_tone_adsr(track, 5500.0, 0.001, 0.070, 0.000, 0.070, "noise", 3.400)
        when "hat" then write_tone_adsr(track, 14000.0, 0.001, 0.030, 0.000, 0.030, "noise", 2.900)
        else raise "unknown preset: #{name}"
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
        raise "step length must be positive" unless step_beats > 0
        emit_arpeggio(track, notes, start_beat, step_beats, duration_beats, velocity)
    end

    def emit_arpeggio(track, notes, start_beat, step_beats, duration_beats, velocity)
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
        raise "bars must be a positive integer" unless bars.is_a?(Integer) && bars >= 1
        validate_step_pattern(pattern, step_beats)
        emit_loop_steps(track, midi, pattern, bars, step_beats, duration_beats, velocity)
    end

    def emit_loop_steps(track, midi, pattern, bars, step_beats, duration_beats, velocity)
        bar_beats = 16 * step_beats
        bar = 0
        while bar < bars
            emit_step_pattern(track, midi, pattern, bar * bar_beats, step_beats, duration_beats, velocity)
            bar += 1
        end
    end

    def validate_step_pattern(pattern, step_beats)
        raise "step pattern must have 16 characters" unless pattern.length == 16
        raise "step length must be positive" unless step_beats > 0
        validate_step_chars(pattern)
    end

    def validate_step_chars(pattern)
        index = 0
        while index < 16
            raise "step pattern accepts only x and -" unless pattern[index] == "x" || pattern[index] == "-"
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
        raise "MIDI note must be an integer from 0 to 127" unless midi.is_a?(Integer) && midi >= 0 && midi <= 127
        if track
            write_track_note(track, midi, beat, duration_beats, velocity)
        else
            printf("{\"type\":\"note\",\"instrument\":\"analog_bass\",\"note\":%d,\"beat\":%.4f,\"duration_beats\":%.4f,\"velocity\":%.3f}\n",
                   midi, beat, duration_beats, velocity)
        end
    end

    def write_track_note(track, midi, beat, duration_beats, velocity)
        validate_track_name(track)
        printf("{\"type\":\"note\",\"instrument\":\"analog_bass\",\"track\":\"%s\",\"note\":%d,\"beat\":%.4f,\"duration_beats\":%.4f,\"velocity\":%.3f}\n",
               track, midi, beat, duration_beats, velocity)
    end

    def midi_for_name(name)
        raise "unknown note: #{name}" unless name.length == 2 || name.length == 3
        semitone = base_semitone(name)
        semitone += 1 if name.length == 3
        octave_digit = note_octave(name)
        midi = (octave_digit.to_i + 1) * 12 + semitone
        raise "note is outside MIDI range: #{name}" unless midi <= 127
        midi
    end

    def base_semitone(name)
        case name[0]
        when "C" then 0
        when "D" then 2
        when "E" then 4
        when "F" then 5
        else upper_semitone(name)
        end
    end

    def upper_semitone(name)
        case name[0]
        when "G" then 7
        when "A" then 9
        when "B" then 11
        else raise "unknown note: #{name}"
        end
    end

    def note_octave(name)
        raise "unknown note: #{name}" if name.length == 3 && name[1] != "#"
        digit = name[name.length - 1]
        raise "unknown note: #{name}" unless digit >= "0" && digit <= "9"
        digit
    end

    def validate_oscillator(oscillator)
        unless oscillator == "saw" || oscillator == "sine" || oscillator == "pulse" || oscillator == "noise"
            raise "unknown oscillator: #{oscillator}"
        end
    end

    def validate_track_name(track)
        raise "invalid track name" unless track.is_a?(String) && track.length >= 1 && track.length <= 32
        index = 0
        while index < track.length
            validate_track_char(track[index])
            index += 1
        end
    end

    def validate_track_char(char)
        return if letter?(char)
        return if track_symbol?(char)
        raise "invalid track name"
    end

    def letter?(char)
        (char >= "A" && char <= "Z") || (char >= "a" && char <= "z")
    end

    def track_symbol?(char)
        (char >= "0" && char <= "9") || char == "_" || char == "-"
    end
end

