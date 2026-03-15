use std::{net::IpAddr, sync::Arc};

use async_trait::async_trait;
use tracing::debug;

use crate::sources::{
    http::HttpTrack,
    playable_track::{PlayableTrack, ResolvedTrack},
};

pub struct AudiusTrack {
    pub client: Arc<reqwest::Client>,
    pub track_id: String,
    pub stream_url: Option<String>,
    pub app_name: String,
    pub local_addr: Option<IpAddr>,
}

const API_BASE: &str = "https://discoveryprovider.audius.co";

#[async_trait]
impl PlayableTrack for AudiusTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let url = if let Some(url) = self.stream_url.clone() {
            url
        } else {
            fetch_stream_url(&self.client, &self.track_id, &self.app_name)
                .await
                .ok_or_else(|| format!("Failed to fetch Audius stream URL for track ID {}", self.track_id))?
        };

        debug!("Audius stream URL: {url}");

        HttpTrack {
            url,
            local_addr: self.local_addr,
            proxy: None,
        }
        .resolve()
        .await
    }
}

pub async fn fetch_stream_url(
    client: &Arc<reqwest::Client>,
    track_id: &str,
    app_name: &str,
) -> Option<String> {
    let url = format!(
        "{API_BASE}/v1/tracks/{}/stream",
        urlencoding::encode(track_id)
    );

    let resp = client
        .get(url)
        .query(&[("app_name", app_name), ("no_redirect", "true")])
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let body: serde_json::Value = resp.json().await.ok()?;
    body["data"].as_str().map(|s| s.to_owned())
}