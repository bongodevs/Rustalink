use std::{
    net::IpAddr,
    sync::{Arc, OnceLock},
};

use async_trait::async_trait;
use regex::Regex;
use tracing::debug;

use crate::sources::{
    http::HttpTrack,
    playable_track::{PlayableTrack, ResolvedTrack},
};

pub struct BandcampTrack {
    pub client: Arc<reqwest::Client>,
    pub uri: String,
    pub stream_url: Option<String>,
    pub local_addr: Option<IpAddr>,
}

pub static STREAM_PATTERN: OnceLock<Regex> = OnceLock::new();

#[async_trait]
impl PlayableTrack for BandcampTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let url = if let Some(url) = self.stream_url.clone() {
            url
        } else {
            fetch_stream_url(&self.client, &self.uri)
                .await
                .ok_or_else(|| format!("Failed to fetch Bandcamp stream URL for {}", self.uri))?
        };

        debug!("Bandcamp stream URL: {url}");

        HttpTrack {
            url,
            local_addr: self.local_addr,
            proxy: None,
        }
        .resolve()
        .await
    }
}

pub async fn fetch_stream_url(client: &Arc<reqwest::Client>, uri: &str) -> Option<String> {
    let resp = client
        .get(uri)
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let body = resp.text().await.ok()?;
    extract_stream_url(&body)
}

pub fn extract_stream_url(body: &str) -> Option<String> {
    STREAM_PATTERN
        .get_or_init(|| Regex::new(r"https?://t4\.bcbits\.com/stream/[a-zA-Z0-9]+/mp3-128/\d+\?p=\d+&amp;ts=\d+&amp;t=[a-zA-Z0-9]+&amp;token=\d+_[a-zA-Z0-9]+").unwrap())
        .find(body)
        .map(|m| m.as_str().replace("&amp;", "&"))
}