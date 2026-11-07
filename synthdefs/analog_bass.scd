(
SynthDef(\analog_bass, { |out = 0, freq = 65.4, amp = 0.1, gain = 1, dur = 0.25, cutoff = 0, attack = 0.01, decay = 0, sustain = 1, release = 0.09, lfoRate = 0, lfoDepth = 0, osc = 0, pan = 0|
    var signal, wave, envelope, effectiveCutoff, movingCutoff, attackTime, decayTime, hold, releaseTime, envelopeScale;
    var isKick, isNoise, isHat, kickFreq, snareFreq, noiseWave, envSustain;
    isKick = (osc >= 0.5) * (osc <= 1.5) * (sustain <= 0.05);
    isNoise = (osc >= 2.5);
    isHat = isNoise * (cutoff >= 8000);
    kickFreq = Select.kr(isKick, [
        freq,
        freq * EnvGen.kr(Env([3.2, 1.35, 1.0], [0.012, 0.055], \exp))
    ]);
    snareFreq = freq * EnvGen.kr(Env([1.8, 1.0], [0.025], \exp));
    noiseWave = Select.ar(isHat, [
        (SinOsc.ar(snareFreq) * EnvGen.kr(Env([1, 0], [0.065])) * 0.95 + HPF.ar(WhiteNoise.ar(1.35), 300)).tanh,
        HPF.ar(WhiteNoise.ar(1.6), 4500).tanh
    ]);
    effectiveCutoff = Select.kr(cutoff <= 0, [cutoff, (freq * 4).clip(80, 2000)]).clip(40, 18000);
    movingCutoff = (effectiveCutoff * SinOsc.kr(lfoRate).range(1 - lfoDepth, 1)).clip(40, 18000);
    wave = Select.ar(osc, [
        Saw.ar(freq),
        Select.ar(isKick, [SinOsc.ar(freq), (SinOsc.ar(kickFreq) * 1.35 + SinOsc.ar(kickFreq * 2, 0, 0.25)).tanh]),
        Pulse.ar(freq, 0.5),
        noiseWave
    ]);
    signal = LPF.ar(wave, movingCutoff);
    envelopeScale = (dur / (attack + decay + release).max(0.000001)).min(1);
    attackTime = Select.kr(cutoff <= 0, [attack * envelopeScale, attack.min(dur * 0.1)]);
    decayTime = decay * envelopeScale;
    releaseTime = Select.kr(cutoff <= 0, [release * envelopeScale, release.min(dur * 0.3)]);
    hold = (dur - attackTime - decayTime - releaseTime).max(0);
    envSustain = Select.kr(isKick, [
        Select.kr(isNoise * (sustain <= 0.05), [sustain, Select.kr(isHat, [0.55, 0.42])]),
        0.72
    ]);
    envelope = EnvGen.kr(Env([0, 1, envSustain, envSustain, 0], [attackTime, decayTime, hold, releaseTime]), doneAction: 2);
    Out.ar(out, Pan2.ar(signal * envelope * amp * gain * 1.41421356, pan));
}).writeDefFile("synthdefs");
0.exit;
)

