"""Render the existing SuperCollider drum voices to WAV using scsynth NRT.

Python writes the OSC score and invokes scsynth; it does not synthesize audio.
Run from any directory after generating synthdefs/analog_bass.scsyndef.
"""
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
# Frozen asset recipes from the drum presets, with durations expressed in seconds.
# name, MIDI, oscillator, cutoff, decay, release, gain, note length, file length
RECIPES = [
    ('kick', 36, 1, 320, .080, .090, 3.8, .18, .35),
    ('snare', 50, 3, 5500, .070, .070, 3.4, .14, .30),
    ('hat', 66, 3, 14000, .030, .030, 2.9, .07, .18),
]


def osc_string(value):
    raw = value.encode() + b'\0'
    return raw + b'\0' * (-len(raw) % 4)


def message(address, *args):
    tags, payload = ',', b''
    for arg in args:
        if isinstance(arg, str):
            tags += 's'
            payload += osc_string(arg)
        elif isinstance(arg, bytes):
            tags += 'b'
            payload += struct.pack('>i', len(arg)) + arg + b'\0' * (-len(arg) % 4)
        elif isinstance(arg, int):
            tags += 'i'
            payload += struct.pack('>i', arg)
        else:
            tags += 'f'
            payload += struct.pack('>f', arg)
    return osc_string(address) + osc_string(tags) + payload


def packet(at, msg):
    # NRT score timetags are elapsed seconds, not wall-clock NTP dates.
    stamp = round(at * (1 << 32))
    bundle = b'#bundle\0' + struct.pack('>Q', stamp) + struct.pack('>i', len(msg)) + msg
    return struct.pack('>i', len(bundle)) + bundle


def main():
    synthdef = ROOT / 'synthdefs/analog_bass.scsyndef'
    if not synthdef.is_file():
        raise SystemExit('Generate the SynthDef first: sclang synthdefs/analog_bass.scd')
    output = ROOT / 'samples'
    output.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='spinel-drums-') as temp:
        for name, midi, osc, cutoff, decay, release, gain, duration, length in RECIPES:
            trigger = message('/s_new', 'analog_bass', 1000, 0, 0,
                              'freq', 440.0 * 2 ** ((midi - 69) / 12),
                              'osc', osc, 'cutoff', float(cutoff),
                              'attack', .001, 'decay', decay, 'sustain', 0.0,
                              'release', release, 'gain', gain, 'dur', duration, 'amp', .12)
            score = Path(temp) / (name + '.osc')
            # NRT completes the load at logical time zero before the completion
            # message creates the synth. No loading silence is baked into the WAV.
            score.write_bytes(packet(0, message('/d_load', str(synthdef), trigger))
                              + packet(length, message('/c_set', 0, 0)))
            rendered = Path(temp) / (name + '.wav')
            subprocess.run(['scsynth', '-N', str(score), '_', str(rendered),
                            '48000', 'WAV', 'int16', '-i', '0', '-o', '2'],
                           check=True, timeout=30)
            (output / rendered.name).write_bytes(rendered.read_bytes())
            print(f'Rendered samples/{name}.wav', flush=True)


if __name__ == '__main__':
    main()
