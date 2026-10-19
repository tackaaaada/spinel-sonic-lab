"""Generated drum asset contract; no audio device or third-party modules needed."""
import array
import math
from pathlib import Path
import sys
import unittest
import wave

ROOT = Path(__file__).resolve().parents[1]


def inspect(path):
    with wave.open(str(path), 'rb') as wav:
        assert (wav.getnchannels(), wav.getsampwidth(), wav.getframerate()) == (2, 2, 48000)
        frames = wav.getnframes()
        data = array.array('h', wav.readframes(frames))
    if sys.byteorder != 'little':
        data.byteswap()
    peak = max(abs(x) for x in data) / 32768
    rms = math.sqrt(sum((x / 32768) ** 2 for x in data) / len(data))
    return frames, data, peak, rms


class DrumAssets(unittest.TestCase):
    def test_wavs_are_short_audible_unclipped_and_finish_in_silence(self):
        for name, duration in [('kick', .35), ('snare', .30), ('hat', .18)]:
            with self.subTest(name=name):
                frames, data, peak, rms = inspect(ROOT / 'samples' / (name + '.wav'))
                self.assertAlmostEqual(frames / 48000, duration, delta=.003)
                self.assertGreater(peak, .05)
                self.assertLess(peak, .95)
                self.assertGreater(rms, .005)
                # A sample trigger should not acquire a loading gap in the asset.
                self.assertGreater(max(abs(x) for x in data[:960]), 100)
                self.assertLessEqual(max(abs(x) for x in data[-960:]), 2)
                print(f'{name}: frames={frames} peak={20*math.log10(peak):.2f}dBFS rms={20*math.log10(rms):.2f}dBFS')


if __name__ == '__main__':
    unittest.main()
