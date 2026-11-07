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
        let metadata = parse_wav(&bytes).ok_or_else(|| invalid_wav(&path))?;
        Ok(Self::from_metadata(path, bytes, metadata))
    }

    fn from_metadata(path: PathBuf, bytes: Vec<u8>, metadata: (u16, u32, u32)) -> Self {
        let (channels, sample_rate, frames) = metadata;
        Self { path, bytes: bytes.into(), channels, frames, sample_rate }
    }
}

fn invalid_wav(path: &std::path::Path) -> String {
    format!("invalid sample WAV: {} (PCM16 mono/stereo, 8..192kHz, <=30s required)", path.display())
}

#[derive(Clone, Copy)]
struct WavFormat {
    channels: u16,
    rate: u32,
    align: u16,
}

struct WavScan<'a> {
    bytes: &'a [u8],
    offset: usize,
    format: Option<WavFormat>,
    data_size: Option<usize>,
}

struct Chunk<'a> {
    id: &'a [u8],
    start: usize,
    end: usize,
    length: usize,
}

fn parse_wav(bytes: &[u8]) -> Option<(u16, u32, u32)> {
    validate_wav_header(bytes)?;
    let mut scan = WavScan::new(bytes);
    scan.scan()?;
    let format = scan.format?;
    let frames = validate_frames(scan.data_size?, format)?;
    Some((format.channels, format.rate, frames))
}

fn validate_wav_header(bytes: &[u8]) -> Option<()> {
    if bytes.len() < 12 || bytes.len() > MAX_SAMPLE_BYTES {
        return None;
    }
    if &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    (u32_at(bytes, 4) as usize + 8 == bytes.len()).then_some(())
}

impl<'a> WavScan<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 12, format: None, data_size: None }
    }

    fn scan(&mut self) -> Option<()> {
        while self.offset < self.bytes.len() {
            let chunk = self.read_chunk()?;
            self.accept(&chunk)?;
            self.advance(&chunk)?;
        }
        Some(())
    }

    fn advance(&mut self, chunk: &Chunk<'_>) -> Option<()> {
        self.offset = chunk.end.checked_add(chunk.length % 2)?;
        (self.offset <= self.bytes.len()).then_some(())
    }

    fn read_chunk(&self) -> Option<Chunk<'a>> {
        let (start, end, length) = chunk_bounds(self.bytes, self.offset)?;
        Some(Chunk { id: &self.bytes[self.offset..self.offset + 4], start, end, length })
    }

    fn accept(&mut self, chunk: &Chunk<'_>) -> Option<()> {
        match chunk.id {
            b"fmt " => self.accept_format(chunk),
            b"data" => self.accept_data(chunk),
            _ => Some(()),
        }
    }

    fn accept_format(&mut self, chunk: &Chunk<'_>) -> Option<()> {
        if self.format.is_some() {
            return None;
        }
        self.format = Some(WavFormat::from_chunk(self.bytes, chunk)?);
        Some(())
    }

    fn accept_data(&mut self, chunk: &Chunk<'_>) -> Option<()> {
        if self.format.is_none() || self.data_size.is_some() {
            return None;
        }
        self.data_size = Some(chunk.length);
        Some(())
    }
}

fn chunk_bounds(bytes: &[u8], offset: usize) -> Option<(usize, usize, usize)> {
    if offset + 8 > bytes.len() {
        return None;
    }
    let length = u32_at(bytes, offset + 4) as usize;
    let start = offset + 8;
    let end = start.checked_add(length)?;
    (end <= bytes.len()).then_some((start, end, length))
}

impl WavFormat {
    fn from_chunk(bytes: &[u8], chunk: &Chunk<'_>) -> Option<Self> {
        if chunk.length < 16 {
            return None;
        }
        let start = chunk.start;
        let format = Self::read_fields(bytes, start);
        format.valid(bytes, start).then_some(format)
    }

    fn read_fields(bytes: &[u8], start: usize) -> Self {
        Self {
            channels: u16_at(bytes, start + 2),
            rate: u32_at(bytes, start + 4),
            align: u16_at(bytes, start + 12),
        }
    }

    fn valid(self, bytes: &[u8], start: usize) -> bool {
        u16_at(bytes, start) == 1
            && (1..=2).contains(&self.channels)
            && u16_at(bytes, start + 14) == 16
            && (8000..=192000).contains(&self.rate)
            && self.align == self.channels * 2
            && u32_at(bytes, start + 8) == self.rate * u32::from(self.align)
    }
}

fn validate_frames(size: usize, format: WavFormat) -> Option<u32> {
    if size == 0 || size % usize::from(format.align) != 0 {
        return None;
    }
    let frames = (size / usize::from(format.align)) as u32;
    (frames <= format.rate * 30).then_some(frames)
}

fn u16_at(bytes: &[u8], i: usize) -> u16 {
    u16::from_le_bytes(bytes[i..i + 2].try_into().unwrap())
}

fn u32_at(bytes: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap())
}
