// Whole-file playback at the file's original sample rate and pitch.
// Mono is duplicated to stereo; stereo preserves its channels. No normalization.
(
SynthDef(\sample_mono, { |out = 0, buf = 0, amp = 1, pan = 0|
    var signal = PlayBuf.ar(1, buf, BufRateScale.kr(buf), loop: 0, doneAction: 2);
    OffsetOut.ar(out, Pan2.ar(signal * amp * 1.41421356, pan));
}).writeDefFile("synthdefs");
SynthDef(\sample_stereo, { |out = 0, buf = 0, amp = 1, pan = 0|
    var signal = PlayBuf.ar(2, buf, BufRateScale.kr(buf), loop: 0, doneAction: 2);
    OffsetOut.ar(out, Balance2.ar(signal[0], signal[1], pan) * amp * 1.41421356);
}).writeDefFile("synthdefs");
0.exit;
)
