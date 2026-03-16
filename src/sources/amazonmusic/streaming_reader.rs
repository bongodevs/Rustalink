use std::{
    io::{Read, Seek, SeekFrom},
    sync::Arc,
    time::Duration,
};

use parking_lot::{Condvar, Mutex};
use symphonia::core::io::MediaSource;
use tracing::{debug, warn};

use super::crypt::{CencDecryptor, extract_flac_stream_header};

struct BufferState {
    data: Vec<u8>,
    available: usize,
    finished: bool,
    error: Option<String>,
}

pub struct AmazonStreamingReader {
    cursor: u64,
    shared: Arc<(Mutex<BufferState>, Condvar)>,
}

impl AmazonStreamingReader {
    pub fn new(
        client: reqwest::Client,
        url: &str,
        decryption_key: &str,
        total_len: u64,
    ) -> Result<Self, String> {
        let decryptor = CencDecryptor::from_hex(decryption_key)?;

        let shared = Arc::new((
            Mutex::new(BufferState {
                data: Vec::new(),
                available: 0,
                finished: false,
                error: None,
            }),
            Condvar::new(),
        ));

        let shared_bg = Arc::clone(&shared);
        let url_owned = url.to_string();

        std::thread::Builder::new()
            .name("amz-flac-extract".into())
            .spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(download_and_extract_flac(
                        shared_bg, client, url_owned, decryptor, total_len,
                    ));
            })
            .map_err(|e| format!("failed to spawn stream thread: {e}"))?;

        {
            let (lock, cvar) = &*shared;
            let mut state = lock.lock();
            while state.available == 0 && !state.finished && state.error.is_none() {
                cvar.wait_for(&mut state, Duration::from_millis(50));
            }
            if let Some(ref e) = state.error {
                return Err(e.clone());
            }
            if state.available == 0 {
                return Err("stream ended before FLAC header could be extracted".into());
            }
        }

        Ok(Self { cursor: 0, shared })
    }
}

impl Read for AmazonStreamingReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (lock, cvar) = &*self.shared;
        let mut state = lock.lock();
        loop {
            if let Some(ref err) = state.error {
                return Err(std::io::Error::other(err.clone()));
            }
            let pos = self.cursor as usize;
            if pos < state.available {
                let n = (state.available - pos).min(buf.len());
                buf[..n].copy_from_slice(&state.data[pos..pos + n]);
                self.cursor += n as u64;
                return Ok(n);
            }
            if state.finished {
                return Ok(0);
            }
            cvar.wait_for(&mut state, Duration::from_millis(50));
        }
    }
}

impl Seek for AmazonStreamingReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let target: u64 = match pos {
            SeekFrom::Start(p) => p,
            SeekFrom::Current(d) => self.cursor.saturating_add_signed(d),
            SeekFrom::End(d) => {
                let (lock, cvar) = &*self.shared;
                let mut state = lock.lock();
                while !state.finished && state.error.is_none() {
                    cvar.wait_for(&mut state, Duration::from_millis(50));
                }
                if let Some(ref e) = state.error {
                    return Err(std::io::Error::other(e.clone()));
                }
                (state.available as i64).saturating_add(d).max(0) as u64
            }
        };

        {
            let (lock, cvar) = &*self.shared;
            let mut state = lock.lock();
            while (state.available as u64) < target && !state.finished && state.error.is_none() {
                cvar.wait_for(&mut state, Duration::from_millis(50));
            }
            if let Some(ref e) = state.error {
                return Err(std::io::Error::other(e.clone()));
            }
            self.cursor = target.min(state.available as u64);
        }

        Ok(self.cursor)
    }
}

impl MediaSource for AmazonStreamingReader {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        let (lock, _) = &*self.shared;
        let state = lock.lock();
        if state.finished {
            Some(state.available as u64)
        } else {
            None
        }
    }
}

impl Drop for AmazonStreamingReader {
    fn drop(&mut self) {
        let (lock, _) = &*self.shared;
        lock.lock().finished = true;
    }
}

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36";

async fn download_and_extract_flac(
    shared: Arc<(Mutex<BufferState>, Condvar)>,
    client: reqwest::Client,
    url: String,
    decryptor: CencDecryptor,
    _total_len: u64,
) {
    let response = match client
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "*/*")
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            signal_error(&shared, format!("HTTP {}", r.status()));
            return;
        }
        Err(e) => {
            signal_error(&shared, format!("request failed: {e}"));
            return;
        }
    };

    let mut staging: Vec<u8> = Vec::with_capacity(256 * 1024);
    let mut total_drained: usize = 0;

    let mut remainder: Vec<u8> = Vec::new();
    let mut stream = response;
    let mut header_written = false;

    loop {
        match stream.chunk().await {
            Ok(Some(chunk)) => {
                staging.extend_from_slice(&chunk);
            }
            Ok(None) => break,
            Err(e) => {
                signal_error(&shared, format!("download error: {e}"));
                return;
            }
        }
        total_drained += process_staging_lowmem(
            &shared,
            &decryptor,
            &mut staging,
            &mut remainder,
            &mut header_written,
        );

        if total_drained > 512 * 1024 {
            staging.shrink_to_fit();
            total_drained = 0;
        }
    }

    process_staging_lowmem(
        &shared,
        &decryptor,
        &mut staging,
        &mut remainder,
        &mut header_written,
    );
    drop(staging);

    if !remainder.is_empty() {
        append_to_buffer(&shared, &remainder);
        remainder.clear();
        remainder.shrink_to_fit();
    }

    let (lock, cvar) = &*shared;
    let mut state = lock.lock();
    state.finished = true;
    cvar.notify_all();
    debug!(
        "Amazon streaming: download complete ({} bytes)",
        state.available
    );
}

fn process_staging_lowmem(
    shared: &Arc<(Mutex<BufferState>, Condvar)>,
    decryptor: &CencDecryptor,
    staging: &mut Vec<u8>,
    remainder: &mut Vec<u8>,
    header_written: &mut bool,
) -> usize {
    let mut total_drained = 0;

    loop {
        if staging.len() < 8 {
            break;
        }

        let box_size =
            u32::from_be_bytes([staging[0], staging[1], staging[2], staging[3]]) as usize;
        if box_size < 8 {
            break;
        }

        let box_type = [staging[4], staging[5], staging[6], staging[7]];

        if &box_type == b"moof" {
            if staging.len() < box_size + 8 {
                break;
            }
            let mdat_size = u32::from_be_bytes([
                staging[box_size],
                staging[box_size + 1],
                staging[box_size + 2],
                staging[box_size + 3],
            ]) as usize;
            let mdat_type = [
                staging[box_size + 4],
                staging[box_size + 5],
                staging[box_size + 6],
                staging[box_size + 7],
            ];
            if &mdat_type != b"mdat" || mdat_size < 8 {
                staging.drain(..box_size);
                total_drained += box_size;
                continue;
            }
            let fragment_total = box_size + mdat_size;
            if staging.len() < fragment_total {
                break;
            }

            let _ = decryptor.decrypt_buffer(&mut staging[..fragment_total]);

            let payload_start = box_size + 8;
            let payload_end = box_size + mdat_size;

            if *header_written && payload_end > payload_start {
                let payload_len = payload_end - payload_start;
                let mut payload_copy = Vec::with_capacity(payload_len);
                payload_copy.extend_from_slice(&staging[payload_start..payload_end]);

                flush_complete_flac_frames(shared, remainder, &payload_copy);
            }

            staging.drain(..fragment_total);
            total_drained += fragment_total;
            continue;
        }

        if staging.len() < box_size {
            break;
        }

        if &box_type == b"moov" && !*header_written {
            match extract_flac_stream_header(&staging[..box_size]) {
                Some(hdr) => {
                    debug!("Amazon FLAC: header extracted ({} bytes)", hdr.len());
                    append_to_buffer(shared, &hdr);
                    *header_written = true;
                }
                None => warn!("Amazon FLAC: failed to extract FLAC header from moov"),
            }
        }

        staging.drain(..box_size);
        total_drained += box_size;
    }

    total_drained
}

fn flush_complete_flac_frames(
    shared: &Arc<(Mutex<BufferState>, Condvar)>,
    remainder: &mut Vec<u8>,
    payload: &[u8],
) {
    remainder.extend_from_slice(payload);

    let mut pos = 0;

    loop {
        if pos >= remainder.len() {
            break;
        }

        match next_flac_frame_end(&remainder[pos..]) {
            Some(frame_len) => {
                append_to_buffer(shared, &remainder[pos..pos + frame_len]);
                pos += frame_len;
            }
            None => {
                break;
            }
        }
    }

    remainder.drain(..pos);
}

fn next_flac_frame_end(data: &[u8]) -> Option<usize> {
    if data.len() < 6 {
        return None;
    }

    if (data[0] != 0xFF) || (data[1] & 0xFE) != 0xF8 {
        if let Some(next) = find_sync(&data[1..]) {
            return next_flac_frame_end(&data[1 + next..]);
        }
        return None;
    }

    let search_from = 6;
    if data.len() <= search_from {
        return None;
    }

    find_sync(&data[search_from..]).map(|offset| search_from + offset)
}

fn find_sync(data: &[u8]) -> Option<usize> {
    data.windows(2)
        .position(|w| w[0] == 0xFF && (w[1] & 0xFE) == 0xF8)
}

fn append_to_buffer(shared: &Arc<(Mutex<BufferState>, Condvar)>, data: &[u8]) {
    let (lock, cvar) = &**shared;
    let mut state = lock.lock();
    state.data.extend_from_slice(data);
    state.available = state.data.len();
    cvar.notify_all();
}

fn signal_error(shared: &Arc<(Mutex<BufferState>, Condvar)>, msg: String) {
    warn!("Amazon FLAC streaming error: {}", msg);
    let (lock, cvar) = &**shared;
    let mut state = lock.lock();
    state.error = Some(msg);
    state.finished = true;
    cvar.notify_all();
}
