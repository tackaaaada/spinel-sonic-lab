//! Ownership of engine buffers, independent of the musical event model.
use crate::sample::{MAX_SCORE_SAMPLE_BYTES, SampleAsset};
use std::time::Duration;

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
        Self {
            backend,
            buffers: Vec::new(),
        }
    }

    /// A failed preparation releases only new allocations; active buffers survive.
    pub fn prepare<'a>(
        &mut self,
        assets: impl IntoIterator<Item = &'a SampleAsset>,
    ) -> Result<(), String> {
        let original = self.buffers.len();
        for asset in assets {
            if self.buffer_id(asset).is_ok() {
                continue;
            }
            let total: usize = self.buffers.iter().map(|b| b.asset.bytes.len()).sum();
            let id = (0..1024).find(|id| self.buffers.iter().all(|b| b.id != *id));
            let result = if total + asset.bytes.len() > MAX_SCORE_SAMPLE_BYTES || id.is_none() {
                Err("sample buffer capacity exceeded (64MiB/1024 buffers)".to_owned())
            } else {
                let id = id.unwrap();
                self.backend
                    .load(id, asset)
                    .map(|()| {
                        self.buffers.push(Buffer {
                            id,
                            asset: asset.clone(),
                            used_until: Duration::ZERO,
                        });
                    })
                    .inspect_err(|_| {
                        let _ = self.backend.free(id);
                    })
            };
            if let Err(error) = result {
                for buffer in self.buffers.drain(original..) {
                    let _ = self.backend.free(buffer.id);
                }
                return Err(error);
            }
        }
        Ok(())
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
            let b = &self.buffers[index];
            if b.used_until <= now && !keep.contains(&b.asset) {
                self.backend.free(b.id)?;
                self.buffers.remove(index);
            } else {
                index += 1;
            }
        }
        Ok(())
    }
}

impl<B: SampleBackend> Drop for SampleBuffers<B> {
    fn drop(&mut self) {
        for b in &self.buffers {
            let _ = self.backend.free(b.id);
        }
    }
}
