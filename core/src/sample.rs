use std::path::PathBuf;
use std::sync::Arc;

/// Validated file snapshot. No buffer numbers or other engine identifiers.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleAsset {
    pub path: PathBuf,
    pub bytes: Arc<[u8]>,
    pub channels: u16,
    pub frames: u32,
    pub sample_rate: u32,
}

impl SampleAsset {
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(f64::from(self.frames) / f64::from(self.sample_rate))
    }
}

pub const MAX_SAMPLE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SCORE_SAMPLE_BYTES: usize = 64 * 1024 * 1024;

impl SampleAsset {
    pub fn from_wav(path: PathBuf, bytes: Vec<u8>) -> Result<Self, String> {
        let invalid = || {
            format!(
                "invalid sample WAV: {} (PCM16 mono/stereo, 8..192kHz, <=30s required)",
                path.display()
            )
        };
        if bytes.len() < 12
            || bytes.len() > MAX_SAMPLE_BYTES
            || &bytes[..4] != b"RIFF"
            || &bytes[8..12] != b"WAVE"
        {
            return Err(invalid());
        }
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap());
        if u32_at(4) as usize + 8 != bytes.len() {
            return Err(invalid());
        }
        let mut format = None;
        let mut data_size = None;
        let mut offset = 12;
        while offset < bytes.len() {
            if offset + 8 > bytes.len() {
                return Err(invalid());
            }
            let length = u32_at(offset + 4) as usize;
            let start = offset + 8;
            let end = start.checked_add(length).ok_or_else(invalid)?;
            if end > bytes.len() {
                return Err(invalid());
            }
            match &bytes[offset..offset + 4] {
                b"fmt " => {
                    if format.is_some() || length < 16 {
                        return Err(invalid());
                    }
                    let short = |i| u16::from_le_bytes(bytes[i..i + 2].try_into().unwrap());
                    let channels = short(start + 2);
                    let rate = u32_at(start + 4);
                    let align = short(start + 12);
                    if short(start) != 1
                        || !(1..=2).contains(&channels)
                        || short(start + 14) != 16
                        || !(8000..=192000).contains(&rate)
                        || align != channels * 2
                        || u32_at(start + 8) != rate * u32::from(align)
                    {
                        return Err(invalid());
                    }
                    format = Some((channels, rate, align));
                }
                b"data" => {
                    if format.is_none() || data_size.replace(length).is_some() {
                        return Err(invalid());
                    }
                }
                _ => {}
            }
            offset = end.checked_add(length % 2).ok_or_else(invalid)?;
            if offset > bytes.len() {
                return Err(invalid());
            }
        }
        let (channels, sample_rate, align) = format.ok_or_else(invalid)?;
        let size = data_size.ok_or_else(invalid)?;
        if size == 0 || size % usize::from(align) != 0 {
            return Err(invalid());
        }
        let frames = (size / usize::from(align)) as u32;
        if frames > sample_rate * 30 {
            return Err(invalid());
        }
        Ok(Self {
            path,
            bytes: bytes.into(),
            channels,
            frames,
            sample_rate,
        })
    }
}
