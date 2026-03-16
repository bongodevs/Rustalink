use std::{net::IpAddr, sync::Arc};

use async_trait::async_trait;
use tracing::debug;

use crate::sources::{
    http::HttpTrack,
    playable_track::{PlayableTrack, ResolvedTrack},
};

pub struct RedditTrack {
    pub client: Arc<reqwest::Client>,
    pub uri: String,
    pub audio_url: Option<String>,
    pub local_addr: Option<IpAddr>,
}

#[async_trait]
impl PlayableTrack for RedditTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let url = self
            .audio_url
            .clone()
            .ok_or_else(|| "No audio stream available for Reddit track".to_string())?;

        debug!("Reddit playback URL: {url}");

        HttpTrack {
            url,
            local_addr: self.local_addr,
            proxy: None,
        }
        .resolve()
        .await
    }
}
