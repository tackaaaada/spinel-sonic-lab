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


def osc_argument(value):
    if isinstance(value, str):
        return 's', osc_string(value)
    if isinstance(value, bytes):
        return 'b', struct.pack('>i', len(value)) + value + b'\0' * (-len(value) % 4)
    if isinstance(value, int):
        return 'i', struct.pack('>i', value)
    return 'f', struct.pack('>f', value)


def message(address, *args):
    tags, payload = ',', b''
    for arg in args:
        tag, encoded = osc_argument(arg)
        tags += tag
        payload += encoded
    return osc_string(address) + osc_string(tags) + payload


def packet(at, msg):
    # NRT score timetags are elapsed seconds, not wall-clock NTP dates.
    stamp = round(at * (1 << 32))
    bundle = b'#bundle\0' + struct.pack('>Q', stamp) + struct.pack('>i', len(msg)) + msg
    return struct.pack('>i', len(bundle)) + bundle


def score_for_recipe(recipe, synthdef):
    _, midi, osc, cutoff, decay, release, gain, duration, length = recipe
    args = ('freq', 440.0 * 2 ** ((midi - 69) / 12), 'osc', osc, 'cutoff', float(cutoff))
    args += ('attack', .001, 'decay', decay, 'sustain', 0.0, 'release', release)
    args += ('gain', gain, 'dur', duration, 'amp', .12)
    trigger = message('/s_new', 'analog_bass', 1000, 0, 0, *args)
    # The load completes at logical time zero, before its completion trigger.
    return packet(0, message('/d_load', str(synthdef), trigger)) + packet(length, message('/c_set', 0, 0))


def render_recipe(recipe, synthdef, output, temp):
    name = recipe[0]
    score = temp / (name + '.osc')
    score.write_bytes(score_for_recipe(recipe, synthdef))
    rendered = temp / (name + '.wav')
    command = ['scsynth', '-N', str(score), '_', str(rendered), '48000', 'WAV', 'int16', '-i', '0', '-o', '2']
    subprocess.run(command, check=True, timeout=30)
    (output / rendered.name).write_bytes(rendered.read_bytes())
    print(f'Rendered samples/{name}.wav', flush=True)


def main():
    synthdef = ROOT / 'synthdefs/analog_bass.scsyndef'
    if not synthdef.is_file():
        raise SystemExit('Generate the SynthDef first: sclang synthdefs/analog_bass.scd')
    output = ROOT / 'samples'
    output.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='spinel-drums-') as temp:
        for recipe in RECIPES:
            render_recipe(recipe, synthdef, output, Path(temp))


if __name__ == '__main__':
    main()
