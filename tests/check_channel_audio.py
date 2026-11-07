"""Render channel pan and the console strip with generated SynthDefs, then inspect WAVs."""

import math
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import wave

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT))
from scripts.render_drum_samples import message, packet  # noqa: E402


def run_nrt(score, output):
    source = output.with_suffix(".osc")
    source.write_bytes(score)
    command = ["scsynth", "-N", str(source), "_", str(output), "48000", "WAV", "int16", "-i", "0", "-o", "2"]
    subprocess.run(command, check=True, capture_output=True, timeout=30)


def read_samples(output):
    with wave.open(str(output), "rb") as wav:
        assert wav.getnchannels() == 2
        return struct.unpack("<" + "h" * wav.getnframes() * 2, wav.readframes(wav.getnframes()))


def render(score, output):
    run_nrt(score, output)
    samples = read_samples(output)
    left = samples[::2]
    right = samples[1::2]
    peak = max(abs(x) for x in samples) / 32768
    rms = math.sqrt(sum((x / 32768) ** 2 for x in samples) / len(samples))
    return left, right, peak, rms


def stop_packet():
    return packet(0.7, message("/c_set", 0, 0))


def synth_score(oscillator, pan, amp):
    trigger = message(
        "/s_new", "analog_bass", 1000, 0, 0,
        "freq", 130.8128, "osc", oscillator, "cutoff", 1800.0,
        "attack", 0.005, "decay", 0.04, "sustain", 0.6,
        "release", 0.06, "gain", 1.0, "dur", 0.4,
        "amp", amp, "pan", pan,
    )
    return packet(0, message("/d_load", str(ROOT / "synthdefs/analog_bass.scsyndef"), trigger)) + stop_packet()


def sample_score(def_name, wav_path, pan):
    trigger = message("/s_new", def_name, 1000, 0, 0, "buf", 7, "amp", 0.4, "pan", pan)
    load = message("/b_allocRead", 7, str(wav_path), 0, -1, trigger)
    return packet(0, message("/d_load", str(ROOT / "synthdefs" / (def_name + ".scsyndef")), load)) + stop_packet()


def db(value):
    return 20 * math.log10(value)


def check_center_oscillators(temp):
    saw_rms = None
    for oscillator, name in enumerate(("Saw", "Sine", "Pulse", "Noise")):
        left, right, peak, rms = render(synth_score(oscillator, 0.0, 0.1), temp / (name + ".wav"))
        assert left == right, f"{name}: center pan changed the legacy stereo balance"
        assert 0.001 < rms < peak < 0.95, f"{name}: invalid level"
        if oscillator == 0:
            saw_rms = rms
        print(f"{name}: peak {db(peak):.2f} dBFS, RMS {db(rms):.2f} dBFS")
    return saw_rms


def check_synth_pan(temp, saw_rms):
    left, right, _, _ = render(synth_score(0, -1.0, 0.1), temp / "synth_left.wav")
    assert max(abs(x) for x in right) == 0 and max(abs(x) for x in left) > 100
    _, _, _, half_rms = render(synth_score(0, 0.0, 0.05), temp / "synth_half.wav")
    assert 0.49 < half_rms / saw_rms < 0.51


def make_mono_sample(temp):
    mono = temp / "mono.wav"
    with wave.open(str(mono), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(48000)
        wav.writeframes(struct.pack("<" + "h" * 4800, *([6000] * 4800)))
    return mono


def check_sample_pan(temp, def_name, path):
    center_left, center_right, center_peak, _ = render(sample_score(def_name, path, 0.0), temp / (def_name + "_center.wav"))
    left, right, peak, rms = render(sample_score(def_name, path, -1.0), temp / (def_name + ".wav"))
    assert max(abs(x) for x in right) == 0 and max(abs(x) for x in left) > 100
    assert center_peak > 0 and max(abs(x) for x in center_right) > 100
    ratio = max(abs(x) for x in left) / max(abs(x) for x in center_left)
    assert 1.40 < ratio < 1.42, f"{def_name}: left pan did not keep center level relationship"
    print(f"{def_name} left: peak {db(peak):.2f} dBFS, RMS {db(rms):.2f} dBFS")


STRIP_DEFAULTS = {
    "inBus": 128, "auxBus": 126, "out": 0, "ha": 1.0, "threshold": 1.0, "ratio": 1.0,
    "attack": 0.01, "release": 0.1, "eqFreq": 1000.0, "eqQ": 1.0, "eqDb": 0.0,
    "sendLevel": 0.0, "pan": 0.0, "fader": 1.0,
}
RETURN_DEFAULTS = {"inBus": 130, "out": 0, "room": 0.8, "damp": 0.3, "wet": 1.0, "fader": 1.0}


def pairs(controls):
    return [item for pair in controls.items() for item in pair]


def sine_source(out, amp):
    return message(
        "/s_new", "analog_bass", 1000, 0, 0, "freq", 200.0, "osc", 1, "cutoff", 0.0,
        "attack", 0.005, "decay", 0.0, "sustain", 1.0, "release", 0.02, "gain", 1.0,
        "dur", 1.0, "amp", amp, "out", out,
    )


def graph_score(strip=None, ret=None, amp=0.2):
    """Groups follow the Rust engine: sources at root head, strips, then returns."""
    score = packet(0, message("/d_loadDir", str(ROOT / "synthdefs")))
    score += packet(0.01, message("/g_new", 1900, 1, 0, 1901, 3, 1900))
    if ret is not None:
        score += packet(0.01, message("/s_new", "aux_return", 2001, 1, 1901, *pairs({**RETURN_DEFAULTS, **ret})))
    if strip is not None:
        score += packet(0.01, message("/s_new", "channel_strip", 2000, 1, 1900, *pairs({**STRIP_DEFAULTS, **strip})))
    score += packet(0.02, sine_source(0 if strip is None else 128, amp))
    return score + packet(3.0, message("/c_set", 0, 0))


def window_rms(samples, start, end):
    part = samples[int(start * 48000) * 2:int(end * 48000) * 2]
    return math.sqrt(sum((x / 32768) ** 2 for x in part) / len(part))


def strip_render(temp, name, strip=None, ret=None, amp=0.2):
    output = temp / (name + ".wav")
    run_nrt(graph_score(strip, ret, amp), output)
    return read_samples(output)


def check_strip_identity(temp):
    direct = window_rms(strip_render(temp, "direct"), 0.3, 0.8)
    strip = window_rms(strip_render(temp, "identity", {}), 0.3, 0.8)
    assert abs(strip / direct - 1) < 0.005, f"bypassed strip changed level: {strip / direct}"
    return direct


def check_compressor(temp):
    loud = {"threshold": 0.1, "attack": 0.005, "release": 0.05}
    bypass = window_rms(strip_render(temp, "comp_bypass", {**loud, "ratio": 1.0}, amp=0.5), 0.3, 0.8)
    squashed = window_rms(strip_render(temp, "comp", {**loud, "ratio": 4.0}, amp=0.5), 0.3, 0.8)
    assert 0.2 < squashed / bypass < 0.5, f"4:1 at -20 dB gave {db(squashed / bypass):.2f} dB"
    trimmed = window_rms(strip_render(temp, "comp_ha", {**loud, "ratio": 4.0, "ha": 3.98}, amp=0.5), 0.3, 0.8)
    assert 1.1 < trimmed / squashed < 2.0, "H/A must feed the compressor before its gain reduction"
    print(f"Compressor 4:1 at -20 dB: {db(squashed / bypass):.2f} dB; +12 dB H/A -> {db(trimmed / squashed):.2f} dB out")


def check_bell(temp, direct):
    boosted = window_rms(strip_render(temp, "bell_on", {"eqFreq": 200.0, "eqDb": 6.0}), 0.3, 0.8)
    away = window_rms(strip_render(temp, "bell_off", {"eqFreq": 5000.0, "eqDb": 6.0}), 0.3, 0.8)
    assert 1.9 < boosted / direct < 2.1, f"+6 dB bell at the tone gave {db(boosted / direct):.2f} dB"
    assert 0.95 < away / direct < 1.05, f"+6 dB bell far from the tone gave {db(away / direct):.2f} dB"
    print(f"Bell +6 dB at 200 Hz: {db(boosted / direct):.2f} dB; at 5 kHz: {db(away / direct):.2f} dB")


def check_aux_is_pre_fader(temp):
    send = {"fader": 0.0, "sendLevel": 1.0, "auxBus": 130}
    wet = strip_render(temp, "aux_pre_fader", send, {})
    dry = strip_render(temp, "aux_off", {**send, "sendLevel": 0.0}, {})
    assert max(abs(x) for x in dry) == 0, "closed fader and send must be silent"
    assert window_rms(wet, 0.3, 0.8) > 0.001, "Aux tap must precede the channel Fader"
    assert window_rms(wet, 1.2, 2.5) > 0.0005, "return reverb must leave a tail"
    print(f"Aux return with fader closed: {db(window_rms(wet, 0.3, 0.8)):.2f} dBFS RMS, tail {db(window_rms(wet, 1.2, 2.5)):.2f} dBFS")


def check_strip_pan(temp):
    left = strip_render(temp, "strip_left", {"pan": -1.0})
    center = strip_render(temp, "strip_center", {})
    assert max(abs(x) for x in left[1::2]) == 0
    ratio = max(abs(x) for x in left[::2]) / max(abs(x) for x in center[::2])
    assert 1.40 < ratio < 1.42, f"strip pan law differs from source pan: {ratio}"


def check_strip(temp):
    direct = check_strip_identity(temp)
    check_compressor(temp)
    check_bell(temp, direct)
    check_aux_is_pre_fader(temp)
    check_strip_pan(temp)


def main():
    with tempfile.TemporaryDirectory(prefix="spinel-channel-audio-") as temp:
        temp = Path(temp)
        check_synth_pan(temp, check_center_oscillators(temp))
        mono = make_mono_sample(temp)
        check_sample_pan(temp, "sample_mono", mono)
        check_sample_pan(temp, "sample_stereo", ROOT / "samples/kick.wav")
        check_strip(temp)


if __name__ == "__main__":
    main()
