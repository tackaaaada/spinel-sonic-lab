"""Characterize the OSC bytes emitted by the sample renderer."""

import hashlib
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.render_drum_samples import message, packet  # noqa: E402
from tests.check_channel_audio import sample_score, synth_score  # noqa: E402


class OscEncoding(unittest.TestCase):
    def test_message_preserves_each_argument_type(self):
        encoded = message("/x", "hi", b"\x01\x02", 7, 0.5)
        self.assertEqual(encoded.hex(), "2f7800002c73626966000000686900000000000201020000000000073f000000")

    def test_packet_uses_elapsed_time_and_length(self):
        encoded = packet(1, message("/x"))
        self.assertEqual(encoded[:16].hex(), "0000001c2362756e646c650000000001")
        self.assertEqual(len(encoded), 32)

    def test_channel_synth_score_bytes(self):
        digest = hashlib.sha256(synth_score(0, 0.0, 0.1)).hexdigest()
        self.assertEqual(digest, "06c9acbbdc5f8692310e85c21f03f36241cbaa82839099697846df906c402ffb")

    def test_channel_sample_score_bytes(self):
        digest = hashlib.sha256(sample_score("sample_mono", Path("/tmp/mono.wav"), -1.0)).hexdigest()
        self.assertEqual(digest, "eab2380eb6cb3c4df8a99b515fca53b818e8c513d06efa0b4d4233434e0d7e50")


if __name__ == "__main__":
    unittest.main()
