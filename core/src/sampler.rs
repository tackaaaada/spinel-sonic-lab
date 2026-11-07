//! Ownership of engine buffers, independent of the musical event model.
use crate::sample::{MAX_SCORE_SAMPLE_BYTES, SampleAsset};
use std::time::Duration;

type Assets<'a> = &'a [SampleAsset];

pub trait SampleBackend {
    fn load(&self, id: i32, asset: &SampleAsset) -> Result<(), String>;
    fn free(&self, id: i32) -> Result<(), String>;
}

/// A buffer to free once every sound reserved through `after` has played.
/// The engine stamps the free on the server timeline, so it cannot overtake a
/// note that a lagging server has not played yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferRelease {
    pub id: i32,
    pub after: Duration,
}

struct Buffer {
    id: i32,
    asset: SampleAsset,
    used_until: Duration,
}

/// scsynth's default buffer count.
const BUFFER_IDS: i32 = 1024;

/// IDs are handed out in rotation, so a released ID returns only after every
/// other free ID: its scheduled free has long run before a new load reuses it.
pub struct SampleBuffers<B: SampleBackend> {
    backend: B,
    buffers: Vec<Buffer>,
    next_id: i32,
}

impl<B: SampleBackend> SampleBuffers<B> {
    pub fn new(backend: B) -> Self {
        Self { backend, buffers: Vec::new(), next_id: 0 }
    }

    /// A failed preparation releases only new allocations; active buffers survive.
    pub fn prepare<'a>(
        &mut self,
        assets: impl IntoIterator<Item = &'a SampleAsset>,
    ) -> Result<(), String> {
        self.prepare_assets(assets)
    }

    fn prepare_assets<'a>(
        &mut self,
        assets: impl IntoIterator<Item = &'a SampleAsset>,
    ) -> Result<(), String> {
        let original = self.buffers.len();
        for asset in assets {
            self.prepare_or_rollback(asset, original)?;
        }
        Ok(())
    }

    fn prepare_or_rollback(&mut self, asset: &SampleAsset, original: usize) -> Result<(), String> {
        if let Err(error) = self.prepare_one(asset) {
            self.rollback(original);
            return Err(error);
        }
        Ok(())
    }

    fn prepare_one(&mut self, asset: &SampleAsset) -> Result<(), String> {
        if self.buffer_id(asset).is_ok() {
            return Ok(());
        }
        let id = self.allocate_id(asset)?;
        self.load_one(id, asset)
    }

    fn allocate_id(&self, asset: &SampleAsset) -> Result<i32, String> {
        let total: usize = self.buffers.iter().map(|b| b.asset.bytes.len()).sum();
        let mut rotation = (0..BUFFER_IDS).map(|step| (self.next_id + step) % BUFFER_IDS);
        let id = rotation.find(|id| self.buffers.iter().all(|b| b.id != *id));
        id.filter(|_| total + asset.bytes.len() <= MAX_SCORE_SAMPLE_BYTES)
            .ok_or_else(|| "sample buffer capacity exceeded (64MiB/1024 buffers)".to_owned())
    }

    fn load_one(&mut self, id: i32, asset: &SampleAsset) -> Result<(), String> {
        if let Err(error) = self.backend.load(id, asset) {
            let _ = self.backend.free(id);
            return Err(error);
        }
        self.buffers.push(Buffer::new(id, asset));
        self.next_id = (id + 1) % BUFFER_IDS;
        Ok(())
    }

    fn rollback(&mut self, original: usize) {
        for buffer in self.buffers.drain(original..) {
            let _ = self.backend.free(buffer.id);
        }
    }

    pub fn buffer_id(&self, asset: &SampleAsset) -> Result<i32, String> {
        self.buffers
            .iter()
            .find(|b| b.asset == *asset)
            .map(|b| b.id)
            .ok_or_else(|| "sample buffer is not loaded".to_owned())
    }

    pub fn mark_used(&mut self, asset: &SampleAsset, end: Duration) -> Result<(), String> {
        let buffer = self
            .buffers
            .iter_mut()
            .find(|b| b.asset == *asset)
            .ok_or("sample buffer is not loaded")?;
        buffer.used_until = buffer.used_until.max(end);
        Ok(())
    }

    /// Removes buffers whose reservations ended by `now` and are not in `keep`;
    /// the caller must schedule each returned free.
    #[must_use]
    pub fn release_finished(&mut self, now: Duration, keep: Assets<'_>) -> Vec<BufferRelease> {
        let finished = |b: &Buffer| b.used_until <= now && !keep.contains(&b.asset);
        let (done, kept) = std::mem::take(&mut self.buffers).into_iter().partition(finished);
        self.buffers = kept;
        done.iter().map(Buffer::release).collect()
    }

    /// Removes every buffer at the end of playback; the caller schedules the frees.
    #[must_use]
    pub fn release_all(&mut self) -> Vec<BufferRelease> {
        self.buffers.drain(..).map(|b| b.release()).collect()
    }
}

impl Buffer {
    fn new(id: i32, asset: &SampleAsset) -> Self {
        Self { id, asset: asset.clone(), used_until: Duration::ZERO }
    }

    fn release(&self) -> BufferRelease {
        BufferRelease { id: self.id, after: self.used_until }
    }
}

impl<B: SampleBackend> Drop for SampleBuffers<B> {
    fn drop(&mut self) {
        for b in &self.buffers {
            let _ = self.backend.free(b.id);
        }
    }
}
