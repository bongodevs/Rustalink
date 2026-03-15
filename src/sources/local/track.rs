use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use async_trait::async_trait;
use tracing::error;

use crate::{
    common::AudioFormat,
    sources::playable_track::{PlayableTrack, ResolvedTrack},
};

pub struct LocalTrack {
    pub path: String,
}

struct LocalFileSource {
    file: std::fs::File,
    len: u64,
}

impl LocalFileSource {
    fn open(path: &str) -> std::io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self { file, len })
    }
}

impl Read for LocalFileSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buf)
    }
}

impl Seek for LocalFileSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos)
    }
}

impl symphonia::core::io::MediaSource for LocalFileSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}

#[async_trait]
impl PlayableTrack for LocalTrack {
    fn supports_seek(&self) -> bool {
        true
    }

    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let path = self.path.clone();

        let hint = Path::new(&path)
            .extension()
            .and_then(|e| e.to_str())
            .map(AudioFormat::from_ext);

        let reader = tokio::task::spawn_blocking(move || {
            LocalFileSource::open(&path)
                .map(|s| Box::new(s) as Box<dyn symphonia::core::io::MediaSource>)
                .map_err(|e| {
                    error!("LocalTrack: failed to open '{path}': {e}");
                    format!("Failed to open file: {e}")
                })
        })
        .await
        .map_err(|e| format!("spawn_blocking failed: {e}"))??;

        Ok(ResolvedTrack::new(reader, hint))
    }
}
