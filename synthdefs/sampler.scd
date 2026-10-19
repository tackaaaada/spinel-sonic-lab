// Whole-file playback at the file's original sample rate and pitch.
// Mono is duplicated to stereo; stereo preserves its channels. No normalization.
(
SynthDef(\sample_mono, { |out = 0, buf = 0, amp = 1|
    var signal = PlayBuf.ar(1, buf, BufRateScale.kr(buf), loop: 0, doneAction: 2);
    OffsetOut.ar(out, (signal * amp) ! 2);
}).writeDefFile("synthdefs");
SynthDef(\sample_stereo, { |out = 0, buf = 0, amp = 1|
    var signal = PlayBuf.ar(2, buf, BufRateScale.kr(buf), loop: 0, doneAction: 2);
    OffsetOut.ar(out, signal * amp);
}).writeDefFile("synthdefs");
0.exit;
)
