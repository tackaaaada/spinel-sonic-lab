//! Ownership of engine buffers, independent of the musical event model.
use crate::sample::{MAX_SCORE_SAMPLE_BYTES, SampleAsset};
use std::time::Duration;

type Assets<'a> = &'a [SampleAsset];
type BufferResult<T> = Result<T, String>;

pub trait SampleBackend {
    fn load(&self, id: i32, asset: &SampleAsset) -> Result<(), String>;
    fn free(&self, id: i32) -> Result<(), String>;
}

struct Buffer {
    id: i32,
    asset: SampleAsset,
    used_until: Duration,
}

pub struct SampleBuffers<B: SampleBackend> {
    backend: B,
    buffers: Vec<Buffer>,
}

impl<B: SampleBackend> SampleBuffers<B> {
    pub fn new(backend: B) -> Self {
        Self { backend, buffers: Vec::new() }
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
        let id = (0..1024).find(|id| self.buffers.iter().all(|b| b.id != *id));
        id.filter(|_| total + asset.bytes.len() <= MAX_SCORE_SAMPLE_BYTES)
            .ok_or_else(|| "sample buffer capacity exceeded (64MiB/1024 buffers)".to_owned())
    }

    fn load_one(&mut self, id: i32, asset: &SampleAsset) -> Result<(), String> {
        if let Err(error) = self.backend.load(id, asset) {
            let _ = self.backend.free(id);
            return Err(error);
        }
        self.buffers.push(Buffer::new(id, asset));
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

    pub fn prune(&mut self, now: Duration, keep: &[SampleAsset]) -> Result<(), String> {
        let mut index = 0;
        while index < self.buffers.len() {
            if !self.discard(index, now, keep)? {
                index += 1;
            }
        }
        Ok(())
    }

    fn discard(&mut self, index: usize, now: Duration, keep: Assets<'_>) -> BufferResult<bool> {
        if self.should_keep(index, now, keep) {
            return Ok(false);
        }
        self.release_index(index)?;
        Ok(true)
    }

    fn should_keep(&self, index: usize, now: Duration, keep: &[SampleAsset]) -> bool {
        let b = &self.buffers[index];
        b.used_until > now || keep.contains(&b.asset)
    }

    fn release_index(&mut self, index: usize) -> Result<(), String> {
        let b = &self.buffers[index];
        self.backend.free(b.id)?;
        self.buffers.remove(index);
        Ok(())
    }
}

impl Buffer {
    fn new(id: i32, asset: &SampleAsset) -> Self {
        Self { id, asset: asset.clone(), used_until: Duration::ZERO }
    }
}

impl<B: SampleBackend> Drop for SampleBuffers<B> {
    fn drop(&mut self) {
        for b in &self.buffers {
            let _ = self.backend.free(b.id);
        }
    }
}
