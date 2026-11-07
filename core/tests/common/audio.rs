//! Level measurements of rendered 48kHz stereo PCM16, in full-scale units.

use spinel_sonic_lab::sample::SampleAsset;

const RATE: f64 = 48_000.0;

pub struct Stereo {
    pub left: Vec<f64>,
    pub right: Vec<f64>,
}

impl Stereo {
    pub fn from_asset(asset: &SampleAsset) -> Self {
        assert_eq!((asset.channels, asset.sample_rate), (2, 48_000), "{}", asset.path.display());
        let samples: Vec<f64> =
            asset.pcm16_samples().iter().map(|s| f64::from(*s) / 32768.0).collect();
        let channel = |offset| samples.iter().skip(offset).step_by(2).copied().collect();
        Self { left: channel(0), right: channel(1) }
    }

    /// The frames between `start` and `end` seconds.
    pub fn window(&self, start: f64, end: f64) -> Self {
        let range = (start * RATE) as usize..(end * RATE) as usize;
        Self { left: self.left[range.clone()].to_vec(), right: self.right[range].to_vec() }
    }

    pub fn peak(&self) -> f64 {
        peak(&self.left).max(peak(&self.right))
    }

    pub fn rms(&self) -> f64 {
        let power = |s: &[f64]| s.iter().map(|x| x * x).sum::<f64>();
        ((power(&self.left) + power(&self.right)) / (2 * self.left.len()) as f64).sqrt()
    }
}

pub fn peak(samples: &[f64]) -> f64 {
    samples.iter().fold(0.0, |max, x| max.max(x.abs()))
}

pub fn db(level: f64) -> f64 {
    20.0 * level.log10()
}

pub fn read_wav(path: &std::path::Path) -> SampleAsset {
    SampleAsset::from_wav(path.to_path_buf(), std::fs::read(path).unwrap()).unwrap()
}

/// A 48kHz PCM16 WAV; `extra` chunks sit between `fmt ` and `data`.
pub fn pcm16_wav(channels: u16, samples: &[i16], extra: &[u8]) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|sample| sample.to_le_bytes()).collect();
    let mut body = b"WAVE".to_vec();
    body.extend(chunk(b"fmt ", &format_fields(channels)));
    body.extend_from_slice(extra);
    body.extend(chunk(b"data", &data));
    chunk(b"RIFF", &body)
}

fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = id.to_vec();
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn format_fields(channels: u16) -> Vec<u8> {
    let align = channels * 2;
    let mut fields = [1, channels].map(u16::to_le_bytes).concat();
    fields.extend([48_000u32, 48_000 * u32::from(align)].map(u32::to_le_bytes).concat());
    fields.extend([align, 16].map(u16::to_le_bytes).concat());
    fields
}
