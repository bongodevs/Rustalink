use std::{net::IpAddr, sync::Arc};

use async_trait::async_trait;

use crate::{
    common::types::AudioFormat,
    config::HttpProxyConfig,
    sources::{
        gaana::crypto::decrypt_stream_path,
        playable_track::{PlayableTrack, ResolvedTrack},
    },
};

pub struct GaanaTrack {
    pub client: Arc<reqwest::Client>,
    pub track_id: String,
    pub stream_quality: String,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for GaanaTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let url = fetch_stream_url_internal(&self.client, &self.track_id, &self.stream_quality)
            .await
            .ok_or_else(|| format!("GaanaTrack: Failed to fetch stream URL for {}", self.track_id))?;

        let local_addr = self.local_addr;
        let proxy      = self.proxy.clone();

        let is_hls = url.contains(".m3u8") || url.contains("/api/manifest/hls_");

        if is_hls {
            crate::sources::youtube::hls::HlsReader::new(&url, local_addr, None, None, proxy)
                .await
                .map(|r| ResolvedTrack::new(
                    Box::new(r) as Box<dyn symphonia::core::io::MediaSource>,
                    Some(AudioFormat::Aac),
                ))
                .map_err(|e| format!("Failed to init HLS reader: {e}"))
        } else {
            let hint = std::path::Path::new(&url)
                .extension()
                .and_then(|s| s.to_str())
                .map(AudioFormat::from_ext);

            super::reader::GaanaReader::new(&url, local_addr, proxy)
                .await
                .map(|r| ResolvedTrack::new(
                    Box::new(r) as Box<dyn symphonia::core::io::MediaSource>,
                    hint,
                ))
                .map_err(|e| format!("Failed to init reader: {e}"))
        }
    }
}

pub(super) async fn fetch_stream_url_internal(
    client: &Arc<reqwest::Client>,
    track_id: &str,
    quality: &str,
) -> Option<String> {
    let body = format!(
        "quality={}&track_id={}&stream_format=mp4",
        urlencoding::encode(quality),
        urlencoding::encode(track_id)
    );

    let resp = client
        .post("https://gaana.com/api/stream-url")
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36")
        .header("Referer", "https://gaana.com/")
        .header("Origin", "https://gaana.com")
        .header("Accept", "application/json, text/plain, */*")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let data: serde_json::Value = resp.json().await.ok()?;
    let encrypted_path = data.get("data")?.get("stream_path")?.as_str()?;

    decrypt_stream_path(encrypted_path)
}