// A persistent stereo strip: source bus -> H/A -> compressor -> bell EQ ->
// Aux tap -> stereo balance -> Fader -> master. Source nodes must run first.
(
SynthDef(\channel_strip, { |inBus = 16, auxBus = 32, out = 0,
    ha = 1, threshold = 0.1, ratio = 1, attack = 0.01, release = 0.1,
    eqFreq = 1000, eqQ = 1, eqDb = 0, sendLevel = 0, pan = 0, fader = 1|
    var signal, detector;
    signal = In.ar(inBus, 2) * ha;
    detector = signal[0].abs.max(signal[1].abs);
    signal = Compander.ar(signal, detector, threshold, 1, 1 / ratio, attack, release);
    signal = BPeakEQ.ar(signal, eqFreq, 1 / eqQ, eqDb);
    Out.ar(auxBus, signal * sendLevel);
    signal = Balance2.ar(signal[0], signal[1], pan) * 1.41421356;
    Out.ar(out, signal * fader);
}).writeDefFile("synthdefs");

// The send tap is post-EQ and pre-Pan/Fader. The return contributes wet sound
// after channel Faders; it has its own Fader and no implicit dry copy.
SynthDef(\aux_return, { |inBus = 32, out = 0, room = 0.35, damp = 0.45, wet = 0.25, fader = 1|
    var signal, reverb;
    signal = In.ar(inBus, 2);
    reverb = FreeVerb2.ar(signal[0], signal[1], 1, room, damp);
    Out.ar(out, reverb * wet * fader);
}).writeDefFile("synthdefs");
0.exit;
)
