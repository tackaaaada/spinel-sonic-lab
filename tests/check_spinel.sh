#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
SPINEL_BIN=${SPINEL_BIN:-spinel}
tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM
for name in bass bright_bass adsr_bass lfo_bass melody_chords step_bass tracks samples techno grid_demo console_grid console_strip; do
    "$SPINEL_BIN" "composer/bin/emit_${name}.rb" -o "$tmpdir/emit_${name}"
    "$tmpdir/emit_${name}" >"$tmpdir/${name}.ndjson"
    diff -u "composer/examples/${name}.ndjson" "$tmpdir/${name}.ndjson"
    echo "Spinel output matches composer/examples/${name}.ndjson"
done
"$SPINEL_BIN" composer/bin/emit_four_bar_kick.rb -o "$tmpdir/emit_four_bar_kick"
"$tmpdir/emit_four_bar_kick" >"$tmpdir/four_bar_kick.ndjson"
diff -u composer/examples/four_bar_kick.ndjson "$tmpdir/four_bar_kick.ndjson"
"$SPINEL_BIN" tests/track_loop.rb -o "$tmpdir/track_loop"
"$tmpdir/track_loop" straight >"$tmpdir/straight_loop.ndjson"
case "$(cat "$tmpdir/straight_loop.ndjson")" in
*'"track":"kick","beat":15.000'*'"velocity":0.650}'*) ;;
*)
    echo "straight four-on-the-floor loop did not reach beat 15" >&2
    exit 1
    ;;
esac
for invalid in bad_bars bad_hit bad_grid bar_outside; do
    if "$tmpdir/track_loop" "$invalid" >"$tmpdir/invalid_loop.ndjson" 2>"$tmpdir/invalid_loop.log"; then
        echo "invalid track loop unexpectedly succeeded: $invalid" >&2
        exit 1
    fi
    if test -s "$tmpdir/invalid_loop.ndjson"; then
        echo "invalid track loop emitted partial score: $invalid" >&2
        exit 1
    fi
done
"$SPINEL_BIN" tests/grid_hits.rb -o "$tmpdir/grid_hits"
for case_spec in "4 15.0000" "8 15.5000" "16 15.7500" "32 15.8750" "64 15.9375"; do
    # Split the known grid and expected beat pair.
    # shellcheck disable=SC2086
    set -- $case_spec
    "$tmpdir/grid_hits" "$1" >"$tmpdir/grid_${1}.ndjson"
    case "$(tail -n 1 "$tmpdir/grid_${1}.ndjson")" in
    *'"beat":'"$2"*) ;;
    *)
        echo "grid $1 did not place the last step at beat $2" >&2
        exit 1
        ;;
    esac
done
"$tmpdir/grid_hits" mixed >"$tmpdir/mixed_grid.ndjson"
case "$(cat "$tmpdir/mixed_grid.ndjson")" in
*'"beat":3.0000'*'"beat":7.5000'*'"beat":11.8750'*'"beat":15.9375'*) ;;
*)
    echo "per-bar grid changes did not retain their beat positions" >&2
    exit 1
    ;;
esac
for invalid in 0 3 128 bad_hit bad_fraction; do
    if "$tmpdir/grid_hits" "$invalid" >"$tmpdir/invalid_grid.ndjson" 2>"$tmpdir/invalid_grid.log"; then
        echo "invalid grid or hit unexpectedly succeeded: $invalid" >&2
        exit 1
    fi
    if test -s "$tmpdir/invalid_grid.ndjson"; then
        echo "invalid grid or hit emitted partial score: $invalid" >&2
        exit 1
    fi
done
"$SPINEL_BIN" tests/grid_melody.rb -o "$tmpdir/grid_melody"
"$tmpdir/grid_melody" >"$tmpdir/grid_melody.ndjson"
case "$(cat "$tmpdir/grid_melody.ndjson")" in
*'"note":48,"beat":0.0000'*'"note":52,"beat":3.0000'*) ;;
*)
    echo "grid melody did not emit the expected MIDI notes" >&2
    exit 1
    ;;
esac
"$SPINEL_BIN" tests/grid_expression.rb -o "$tmpdir/grid_expression"
"$tmpdir/grid_expression" >"$tmpdir/grid_expression.ndjson"
diff -u composer/examples/grid_expression.ndjson "$tmpdir/grid_expression.ndjson"
case "$(cat "$tmpdir/grid_expression.ndjson")" in
*'"sample":"kick","track":"kick","beat":0.0000,"velocity":0.400}'*'"note":48,"beat":0.0000,"duration_beats":0.5000,"velocity":0.700}'*'"note":52,"beat":2.0000,"duration_beats":1.0000,"velocity":0.800}'*) ;;
*)
    echo "Grid expression did not preserve velocity, MIDI pitch, beat, and duration" >&2
    exit 1
    ;;
esac
for invalid in sample_duration bad_velocity bad_duration; do
    if "$tmpdir/grid_expression" "$invalid" >"$tmpdir/invalid_grid_expression.ndjson" 2>"$tmpdir/invalid_grid_expression.log"; then
        echo "invalid Grid expression unexpectedly succeeded: $invalid" >&2
        exit 1
    fi
    if test -s "$tmpdir/invalid_grid_expression.ndjson"; then
        echo "invalid Grid expression emitted partial score: $invalid" >&2
        exit 1
    fi
done
"$SPINEL_BIN" tests/console_grid.rb -o "$tmpdir/console_grid"
"$tmpdir/console_grid" >"$tmpdir/console_grid.ndjson"
diff -u composer/examples/console_grid.ndjson "$tmpdir/console_grid.ndjson"
for invalid in bad_order bad_pan; do
    if "$tmpdir/console_grid" "$invalid" >"$tmpdir/invalid_console_grid.ndjson" 2>"$tmpdir/invalid_console_grid.log"; then
        echo "invalid console Grid unexpectedly succeeded: $invalid" >&2
        exit 1
    fi
    if test -s "$tmpdir/invalid_console_grid.ndjson"; then
        echo "invalid console Grid emitted partial score: $invalid" >&2
        exit 1
    fi
done
"$SPINEL_BIN" tests/console_full.rb -o "$tmpdir/console_full"
"$tmpdir/console_full" >"$tmpdir/console_full.ndjson"
diff -u composer/examples/console_full.ndjson "$tmpdir/console_full.ndjson"
for invalid in bad_order missing_return; do
    if "$tmpdir/console_full" "$invalid" >"$tmpdir/invalid_console_full.ndjson" 2>"$tmpdir/invalid_console_full.log"; then
        echo "invalid console module unexpectedly succeeded: $invalid" >&2
        exit 1
    fi
    if test -s "$tmpdir/invalid_console_full.ndjson"; then
        echo "invalid console module emitted partial score: $invalid" >&2
        exit 1
    fi
done
"$SPINEL_BIN" tests/track_names.rb -o "$tmpdir/track_names"
"$tmpdir/track_names" "lead_2-v1" >"$tmpdir/valid_track.ndjson"
case "$(cat "$tmpdir/valid_track.ndjson")" in
*'"track":"lead_2-v1"'*) ;;
*)
    echo "valid track name was not emitted" >&2
    exit 1
    ;;
esac
for invalid in "" "bad/name" "white space" "abcdefghijklmnopqrstuvwxyz123456789"; do
    if "$tmpdir/track_names" "$invalid" >"$tmpdir/invalid_track.ndjson" 2>"$tmpdir/invalid_track.log"; then
        echo "invalid track name unexpectedly succeeded: $invalid" >&2
        exit 1
    fi
    case "$(cat "$tmpdir/invalid_track.ndjson")" in
    *'"type":"note"'*)
        echo "invalid track emitted partial note events" >&2
        exit 1
        ;;
    esac
    if "$tmpdir/track_names" preset "$invalid" soft >"$tmpdir/invalid_track_preset.ndjson" 2>"$tmpdir/invalid_track.log"; then
        echo "invalid track preset name unexpectedly succeeded: $invalid" >&2
        exit 1
    fi
    case "$(cat "$tmpdir/invalid_track_preset.ndjson")" in
    *'"type":"tone"'* | *'"type":"note"'*)
        echo "invalid track preset emitted partial events" >&2
        exit 1
        ;;
    esac
done
"$tmpdir/track_names" preset "bass" "wobble" >"$tmpdir/valid_track_preset.ndjson"
case "$(cat "$tmpdir/valid_track_preset.ndjson")" in
*'"type":"tone","instrument":"analog_bass","track":"bass"'*'"track":"chords","note":52'*) ;;
*)
    echo "track_preset / track_chord output did not match expected NDJSON" >&2
    exit 1
    ;;
esac
"$tmpdir/track_names" drums "kick" >"$tmpdir/valid_drums.ndjson"
case "$(cat "$tmpdir/valid_drums.ndjson")" in
*'"track":"snare","oscillator":"noise"'*'"track":"hat","oscillator":"noise"'*'"track":"kick","note":36'*) ;;
*)
    echo "drum presets / drum_steps output did not match expected NDJSON" >&2
    exit 1
    ;;
esac
if "$tmpdir/track_names" drums "cowbell" >"$tmpdir/invalid_drum.ndjson" 2>"$tmpdir/invalid_drum.log"; then
    echo "invalid drum name unexpectedly succeeded" >&2
    exit 1
fi
case "$(cat "$tmpdir/invalid_drum.ndjson")" in
*'"type":"note"'*)
    echo "invalid drum emitted partial note events" >&2
    exit 1
    ;;
esac
"$SPINEL_BIN" composer/bin/emit_wave_bass.rb -o "$tmpdir/emit_wave_bass"
for oscillator in sine pulse; do
    "$tmpdir/emit_wave_bass" "$oscillator" >"$tmpdir/${oscillator}_bass.ndjson"
    diff -u "composer/examples/${oscillator}_bass.ndjson" "$tmpdir/${oscillator}_bass.ndjson"
    echo "Spinel output matches composer/examples/${oscillator}_bass.ndjson"
done
"$SPINEL_BIN" composer/bin/emit_preset_bass.rb -o "$tmpdir/emit_preset_bass"
for pair in "soft soft_preset" "bright bright_bass" "wobble lfo_bass"; do
    # Split each known pair into the preset and fixture names.
    # shellcheck disable=SC2086
    set -- $pair
    "$tmpdir/emit_preset_bass" "$1" >"$tmpdir/${1}_preset.ndjson"
    diff -u "composer/examples/${2}.ndjson" "$tmpdir/${1}_preset.ndjson"
    echo "Spinel preset $1 matches composer/examples/${2}.ndjson"
done
"$tmpdir/emit_preset_bass" >"$tmpdir/default_preset.ndjson"
diff -u composer/examples/soft_preset.ndjson "$tmpdir/default_preset.ndjson"
if "$tmpdir/emit_preset_bass" unknown >"$tmpdir/invalid_preset.log" 2>&1; then
    echo "invalid preset unexpectedly succeeded" >&2
    exit 1
fi
"$SPINEL_BIN" tests/note_names.rb -o "$tmpdir/note_names"
"$tmpdir/note_names" "C#4" >"$tmpdir/sharp_note.ndjson"
case "$(cat "$tmpdir/sharp_note.ndjson")" in
*'"note":61'*) ;;
*)
    echo "C#4 did not become MIDI note 61" >&2
    exit 1
    ;;
esac
for invalid in H4 Db4 C10 G#9; do
    if "$tmpdir/note_names" "$invalid" >"$tmpdir/invalid_note.ndjson" 2>"$tmpdir/invalid_note.log"; then
        echo "invalid note $invalid unexpectedly succeeded" >&2
        exit 1
    fi
done
if "$tmpdir/note_names" chord H3 >"$tmpdir/invalid_chord.ndjson" 2>"$tmpdir/invalid_chord.log"; then
    echo "invalid chord unexpectedly succeeded" >&2
    exit 1
fi
case "$(cat "$tmpdir/invalid_chord.ndjson")" in
*'"type":"note"'*)
    echo "invalid chord emitted partial note events" >&2
    exit 1
    ;;
esac
"$tmpdir/note_names" arpeggio E4 >"$tmpdir/valid_arpeggio.ndjson"
case "$(cat "$tmpdir/valid_arpeggio.ndjson")" in
*'"track":"lead","note":60,"beat":0.000'*'"track":"lead","note":64,"beat":0.250'*'"track":"lead","note":67,"beat":0.500'*) ;;
*)
    echo "track_arpeggio output did not match expected sequential notes" >&2
    exit 1
    ;;
esac
for invalid_arp in "arpeggio H4" "bad_arpeggio_step"; do
    # Split the known invalid case into command arguments.
    # shellcheck disable=SC2086
    set -- $invalid_arp
    if "$tmpdir/note_names" "$@" >"$tmpdir/invalid_arpeggio.ndjson" 2>"$tmpdir/invalid_arpeggio.log"; then
        echo "invalid arpeggio ($invalid_arp) unexpectedly succeeded" >&2
        exit 1
    fi
    case "$(cat "$tmpdir/invalid_arpeggio.ndjson")" in
    *'"type":"note"'*)
        echo "invalid arpeggio emitted partial note events" >&2
        exit 1
        ;;
    esac
done
"$SPINEL_BIN" tests/step_patterns.rb -o "$tmpdir/step_patterns"
"$tmpdir/step_patterns" loop >"$tmpdir/valid_loop.ndjson"
case "$(cat "$tmpdir/valid_loop.ndjson")" in
*'"track":"bass","note":36,"beat":0.000'*'"track":"bass","note":36,"beat":4.000'*'"track":"kick","note":36,"beat":7.000'*) ;;
*)
    echo "track_loop_steps / drum_loop_steps output did not span two bars" >&2
    exit 1
    ;;
esac
for invalid in bad_length bad_symbol bad_note bad_grid bad_bars; do
    if "$tmpdir/step_patterns" "$invalid" >"$tmpdir/invalid_steps.ndjson" 2>"$tmpdir/invalid_steps.log"; then
        echo "invalid step pattern $invalid unexpectedly succeeded" >&2
        exit 1
    fi
    case "$(cat "$tmpdir/invalid_steps.ndjson")" in
    *'"type":"note"'*)
        echo "invalid step pattern emitted partial note events" >&2
        exit 1
        ;;
    esac
done
"$SPINEL_BIN" tests/tone_oscillators.rb -o "$tmpdir/tone_oscillators"
for variant in tone adsr lfo; do
    if "$tmpdir/tone_oscillators" "$variant" >"$tmpdir/invalid_tone.ndjson" 2>"$tmpdir/invalid_tone.log"; then
        echo "$variant accepted an unsupported oscillator" >&2
        exit 1
    fi
    case "$(cat "$tmpdir/invalid_tone.ndjson")" in
    *'"type":"tone"'*)
        echo "$variant emitted an invalid tone" >&2
        exit 1
        ;;
    esac
done

"$SPINEL_BIN" tests/sample_events.rb -o "$tmpdir/sample_events"
"$tmpdir/sample_events" >"$tmpdir/sample_events.ndjson"
python3 - "$tmpdir/sample_events.ndjson" <<'PY_CHECK'
import json, sys
with open(sys.argv[1]) as f:
    events = [json.loads(line) for line in f]
assert events[1]['path'] == 'a path/quote"and\\slash.wav'
assert events[2]['type'] == 'sample_note'
assert events[2]['beat'] == 0
PY_CHECK
for invalid in bad_name bad_path bad_pattern; do
    if "$tmpdir/sample_events" "$invalid" >"$tmpdir/invalid_sample.ndjson" 2>"$tmpdir/invalid_sample.log"; then
        echo "invalid sample unexpectedly succeeded: $invalid" >&2
        exit 1
    fi
    case "$(cat "$tmpdir/invalid_sample.ndjson")" in
    *'"type":"sample_note"'*)
        echo "invalid sample emitted partial playback events" >&2
        exit 1
        ;;
    esac
done
