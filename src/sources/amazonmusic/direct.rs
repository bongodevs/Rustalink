use std::net::IpAddr;

use async_trait::async_trait;
use tracing::debug;

use super::streaming_reader::AmazonStreamingReader;
use crate::{
    audio::source::create_client,
    config::HttpProxyConfig,
    sources::playable_track::{PlayableTrack, ResolvedTrack},
};

pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36";

pub struct AmazonMusicTrack {
    pub track_id: String,
    pub stream_url: String,
    pub decryption_key: String,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for AmazonMusicTrack {
    fn supports_seek(&self) -> bool {
        true
    }

    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        debug!(
            "Amazon Music: opening streaming reader for {}",
            self.track_id
        );

        let stream_client = create_client(UA.to_owned(), self.local_addr, self.proxy.clone(), None)
            .map_err(|e| format!("failed to create client: {e}"))?;

        let content_len = probe_content_length(&stream_client, &self.stream_url)
            .await
            .map_err(|e| format!("failed to probe stream length: {e}"))?;

        let streaming_reader = AmazonStreamingReader::new(
            stream_client,
            &self.stream_url,
            &self.decryption_key,
            content_len,
        )
        .map_err(|e| format!("failed to initialize streaming reader: {e}"))?;

        let reader = Box::new(streaming_reader) as Box<dyn symphonia::core::io::MediaSource>;

        Ok(ResolvedTrack::new(reader, None))
    }
}

async fn probe_content_length(client: &reqwest::Client, url: &str) -> Result<u64, String> {
    let head = client
        .head(url)
        .header("User-Agent", UA)
        .send()
        .await
        .map_err(|e| format!("HEAD request failed: {e}"))?;

    if let Some(len) = head.content_length() {
        return Ok(len);
    }

    let range_resp = client
        .get(url)
        .header("User-Agent", UA)
        .header("Range", "bytes=0-0")
        .send()
        .await
        .map_err(|e| format!("range probe failed: {e}"))?;

    range_resp
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split('/').next_back())
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| "could not determine stream length".to_string())
}
