# Developing on Omarchy

[日本語](development-omarchy.ja.md)

Notes for working on Spinel Sonic Lab on Omarchy (Linux, PipeWire). For macOS see [usage.md](usage.md#platform-notes).

## Setup used

- Bash; `mise.toml` selects Ruby 4.0.1 and Rust 1.99.1.
- Spinel is built from source into `~/Projects/.tools/spinel/bin/spinel`; mise sets `SPINEL_BIN`. Never use CRuby in place of Spinel to run compositions.
- SuperCollider 3.14.1, with audio going through PipeWire. The audio task sets `SC_JACK_DEFAULT_OUTPUTS=system` so ports auto-connect.

## First-time audio setup

Add the JACK compatibility library (asks for admin authentication):

```sh
sudo pacman -S --needed pipewire-jack
```

Answer `y` when asked to replace `jack2`. On a dependency error, read the message instead of forcing removal.

## Check state and run tests

```sh
mise run doctor        # tools, PipeWire-JACK, audio services
mise run test          # Rust tests + real Spinel compositions
mise run check-score   # compile the 4-note sample and print the schedule, no sound
```

## Playing

Terminal 1: `mise run audio`, and wait for `SuperCollider 3 server ready`. It listens on UDP 127.0.0.1:57110 with stereo output.

Terminal 2: `mise run play`.

Start with the device volume low. The CLI's `scsynth accepted ... note commands` only means the commands were accepted, not that anyone heard sound.

### Command succeeds but nothing is audible

With a directly started `scsynth -u 57110`, SuperCollider's outputs may not be connected to a PipeWire sink. Check `pw-link -l` for `SuperCollider:out_1` and `out_2`. If unconnected:

```sh
sink=$(pactl get-default-sink)
pw-link SuperCollider:out_1 "${sink}:playback_FL"
pw-link SuperCollider:out_2 "${sink}:playback_FR"
```

On 2026-11-01 the CLI reported 56 accepted notes yet nothing was heard until this connection was made. The links belong to the running server; re-check after restarting it.

## Live editing

With the audio server running, in another terminal `mise run watch`, and in a third pane edit `composer/bin/live_session.rb`. A valid change takes effect at the next 4-beat bar boundary; an invalid one keeps the previous score playing. The author heard a cutoff change about 2–3 beats after saving — a listening check, not a measurement of the switch time.

Stop playback and the audio server with Ctrl+C in their terminals.

## sclang and Qt

The `synthdef` task sets `QT_QPA_PLATFORM=offscreen` and `QT_QPA_PLATFORMTHEME=generic` so a desktop `gtk3` theme does not make GTK try to reach a display (a 2026-10-19 sclang SIGABRT showed `gtk_init` → `libqgtk3` → `QPlatformThemeFactory`). `play`, `watch` and `samples` depend on that task. Do not set these on macOS.

`Couldn't set realtime scheduling priority` is a privilege warning, not a failure; check the command's exit status and that the SynthDefs were updated. In restricted sandboxes sclang can fail on `_LocalIPs` permissions, so a full SynthDef regeneration is verified on the host.

## Verification status (as of 2026-11-01)

- All Rust tests and the Spinel composition checks pass on Linux; the two UDP timing tests pass with host privileges.
- `mise run check-score` and `mise run synthdef` succeed.
- After connecting SuperCollider's outputs to the default sink, the author heard the 4-bar sampler example, and judged the techno sketch's kick/bass balance OK for now.
