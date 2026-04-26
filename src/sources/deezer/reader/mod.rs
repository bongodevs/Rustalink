use crate::common::types::AnyResult;
pub mod crypt;
pub mod remote_reader;

use std::io::{Read, Seek, SeekFrom};

use symphonia::core::io::MediaSource;
use tracing::{debug, warn};

use self::{
    crypt::{CHUNK_SIZE, DeezerCrypt},
    remote_reader::DeezerRemoteReader,
};

pub struct DeezerReader {
    source: DeezerRemoteReader,
    crypt: DeezerCrypt,
    pos: u64,
    raw_buf: Vec<u8>,
    ready_buf: Vec<u8>,
    skip_pending: usize,
    decrypt_failures: u32,
}

impl DeezerReader {
    pub async fn new(
        url: &str,
        track_id: &str,
        master_key: &str,
        local_addr: Option<std::net::IpAddr>,
        proxy: Option<crate::config::HttpProxyConfig>,
    ) -> AnyResult<Self> {
        debug!("Initializing DeezerReader for track {}", track_id);

        let source = DeezerRemoteReader::new(url, local_addr, proxy).await?;
        let crypt = DeezerCrypt::new(track_id, master_key);

        Ok(Self {
            source,
            crypt,
            pos: 0,
            raw_buf: Vec::with_capacity(CHUNK_SIZE),
            ready_buf: Vec::with_capacity(CHUNK_SIZE),
            skip_pending: 0,
            decrypt_failures: 0,
        })
    }
}

impl Read for DeezerReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            if self.skip_pending > 0 && !self.ready_buf.is_empty() {
                let to_skip = std::cmp::min(self.skip_pending, self.ready_buf.len());
                self.ready_buf.drain(..to_skip);
                self.skip_pending -= to_skip;
            }

            if self.skip_pending == 0 && !self.ready_buf.is_empty() {
                let n = std::cmp::min(buf.len(), self.ready_buf.len());
                buf[..n].copy_from_slice(&self.ready_buf[..n]);
                self.ready_buf.drain(..n);
                return Ok(n);
            }

            let mut tmp = [0u8; CHUNK_SIZE];
            let n = match self.source.read(&mut tmp) {
                Ok(n) => n,
                Err(e) => {
                    warn!("DeezerReader: failed to read from CDN: {}", e);
                    return Err(e);
                }
            };

            if n == 0 {
                if self.raw_buf.is_empty() {
                    return Ok(0);
                }
                let leftovers = self.raw_buf.clone();
                let chunk_idx = self.pos / CHUNK_SIZE as u64;
                self.crypt
                    .decrypt_chunk(chunk_idx, &leftovers, &mut self.ready_buf);
                self.pos += leftovers.len() as u64;
                self.raw_buf.clear();
                continue;
            }

            self.raw_buf.extend_from_slice(&tmp[..n]);

            while self.raw_buf.len() >= CHUNK_SIZE {
                let chunk: Vec<u8> = self.raw_buf.drain(..CHUNK_SIZE).collect();
                let chunk_idx = self.pos / CHUNK_SIZE as u64;

                let before_len = self.ready_buf.len();
                self.crypt
                    .decrypt_chunk(chunk_idx, &chunk, &mut self.ready_buf);

                if self.ready_buf.len() == before_len {
                    self.decrypt_failures += 1;
                    if self.decrypt_failures % 10 == 1 {
                        warn!(
                            "DeezerReader: {} decryption failures (chunk {}), track may be corrupted or key invalid",
                            self.decrypt_failures, chunk_idx
                        );
                    }
                }

                self.pos += CHUNK_SIZE as u64;
            }
        }
    }
}

impl Seek for DeezerReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(p) => p,
            SeekFrom::Current(0) => {
                let buffered = self.ready_buf.len() as u64 + self.raw_buf.len() as u64;
                return Ok(self.pos.saturating_sub(buffered));
            }
            _ => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Only SeekFrom::Start is supported",
                ));
            }
        };

        let aligned_pos = (target / CHUNK_SIZE as u64) * CHUNK_SIZE as u64;
        let skip = (target - aligned_pos) as usize;

        let new_pos = self.source.seek(SeekFrom::Start(aligned_pos))?;

        self.pos = new_pos;
        self.raw_buf.clear();
        self.ready_buf.clear();
        self.skip_pending = skip;

        Ok(target)
    }
}

impl MediaSource for DeezerReader {
    fn is_seekable(&self) -> bool {
        self.source.is_seekable()
    }

    fn byte_len(&self) -> Option<u64> {
        self.source.byte_len()
    }
}
