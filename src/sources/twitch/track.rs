use std::{
    collections::HashSet,
    io::{self, Read, Seek, SeekFrom},
    net::IpAddr,
};

use async_trait::async_trait;
use symphonia::core::io::MediaSource;

use crate::{
    common::types::AudioFormat,
    config::HttpProxyConfig,
    sources::{
        playable_track::{PlayableTrack, ResolvedTrack},
        youtube::hls::{
            fetcher::fetch_segment_into, resolver::fetch_text, ts_demux::extract_adts_from_ts,
            types::Resource, utils::resolve_url,
        },
    },
};

pub struct TwitchTrack {
    pub stream_url: String,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for TwitchTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let handle = tokio::runtime::Handle::current();

        let (err_tx, _err_rx) = flume::bounded::<String>(1);

        let reader = Box::new(LiveHlsReader::new(
            self.stream_url.clone(),
            self.local_addr,
            self.proxy.clone(),
            handle,
            err_tx,
        ).await) as Box<dyn MediaSource>;

        Ok(ResolvedTrack::new(reader, Some(AudioFormat::Aac)))
    }
}

struct LiveHlsReader {
    chunk_rx: flume::Receiver<Vec<u8>>,
    current: Vec<u8>,
    pos: usize,
}

impl LiveHlsReader {
    pub async fn new(
        manifest_url: String,
        local_addr: Option<IpAddr>,
        proxy: Option<HttpProxyConfig>,
        _handle: tokio::runtime::Handle,
        err_tx: flume::Sender<String>,
    ) -> Self {
        let (chunk_tx, chunk_rx) = flume::bounded::<Vec<u8>>(16);

        tokio::spawn(async move {
            let mut builder =
                reqwest::Client::builder().timeout(std::time::Duration::from_secs(15));

            if let Some(ip) = local_addr {
                builder = builder.local_address(ip);
            }

            if let Some(ref cfg) = proxy
                && let Some(ref url) = cfg.url
            {
                match reqwest::Proxy::all(url) {
                    Ok(mut p) => {
                        if let (Some(u), Some(pw)) = (&cfg.username, &cfg.password) {
                            p = p.basic_auth(u, pw);
                        }
                        builder = builder.proxy(p);
                    }
                    Err(e) => {
                        tracing::error!("Twitch live HLS: proxy setup failed for {url}: {e}");
                        let _ = err_tx.send(format!("Proxy setup failed: {e}"));
                        return;
                    }
                }
            }

            let client = match builder.build() {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!("Twitch live HLS: client build failed: {e}");
                    let _ = err_tx.send(format!("Client build failed: {e}"));
                    return;
                }
            };

            let mut seen: HashSet<String> = HashSet::new();
            let mut seen_history: std::collections::VecDeque<String> =
                std::collections::VecDeque::with_capacity(50);

            loop {
                let text = match fetch_text(&client, &manifest_url).await {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!("Twitch: live playlist refresh failed: {e}");
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        continue;
                    }
                };

                let (segments, target_duration) = parse_live_playlist(&text, &manifest_url);

                for seg in segments {
                    if seen.contains(&seg.url) {
                        continue;
                    }

                    let mut raw = Vec::new();
                    if let Err(e) = fetch_segment_into(&client, &seg, &mut raw).await {
                        tracing::warn!("Twitch: segment fetch error: {e}");
                        continue;
                    }

                    let payload = if raw.first() == Some(&0x47) {
                        let adts = extract_adts_from_ts(&raw);
                        if adts.is_empty() {
                            tracing::debug!("Twitch: ADTS extraction failed, skipping segment");
                            continue;
                        }
                        adts
                    } else {
                        raw
                    };

                    if chunk_tx.send(payload).is_err() {
                        return;
                    }

                    if seen.insert(seg.url.clone()) {
                        seen_history.push_back(seg.url);
                        if seen_history.len() > 50
                            && let Some(old) = seen_history.pop_front()
                        {
                            seen.remove(&old);
                        }
                    }
                }

                let wait = (target_duration / 2.0).max(1.0);
                tokio::time::sleep(std::time::Duration::from_secs_f64(wait)).await;
            }
        });

        Self {
            chunk_rx,
            current: Vec::new(),
            pos: 0,
        }
    }
}

fn parse_live_playlist(text: &str, base_url: &str) -> (Vec<Resource>, f64) {
    let mut segments = Vec::new();
    let mut target_duration = 6.0f64;
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if let Some(rest) = line.strip_prefix("#EXT-X-TARGETDURATION:") {
            if let Ok(d) = rest.trim().parse::<f64>() {
                target_duration = d;
            }
        } else if line.starts_with("#EXTINF:") {
            let duration = line
                .strip_prefix("#EXTINF:")
                .and_then(|r| r.split(',').next())
                .and_then(|d| d.trim().parse::<f64>().ok());

            let mut j = i + 1;
            while j < lines.len() && lines[j].starts_with('#') {
                j += 1;
            }
            if j < lines.len() && !lines[j].is_empty() {
                let url = resolve_url(base_url, lines[j]);
                segments.push(Resource {
                    url,
                    range: None,
                    duration,
                });
            }
            i = j + 1;
            continue;
        }

        i += 1;
    }

    (segments, target_duration)
}

impl Read for LiveHlsReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.pos < self.current.len() {
                let n = buf.len().min(self.current.len() - self.pos);
                buf[..n].copy_from_slice(&self.current[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }

            match self.chunk_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(chunk) => {
                    self.current = chunk;
                    self.pos = 0;
                }
                Err(flume::RecvTimeoutError::Timeout) => continue,
                Err(flume::RecvTimeoutError::Disconnected) => return Ok(0),
            }
        }
    }
}

impl Seek for LiveHlsReader {
    fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "live streams are not seekable",
        ))
    }
}

impl MediaSource for LiveHlsReader {
    fn is_seekable(&self) -> bool {
        false
    }

    fn byte_len(&self) -> Option<u64> {
        None
    }
}