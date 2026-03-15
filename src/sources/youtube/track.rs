use std::{net::IpAddr, sync::Arc};

use async_trait::async_trait;
use tracing::{debug, error, info, warn};

use crate::{
    config::HttpProxyConfig,
    sources::{
        playable_track::{PlayableTrack, ResolvedTrack},
        youtube::{
            cipher::YouTubeCipherManager,
            clients::YouTubeClient,
            oauth::YouTubeOAuth,
            utils::{create_reader, detect_audio_kind},
        },
    },
};

pub struct YoutubeTrack {
    pub identifier: String,
    pub clients: Vec<Arc<dyn YouTubeClient>>,
    pub oauth: Arc<YouTubeOAuth>,
    pub cipher_manager: Arc<YouTubeCipherManager>,
    pub visitor_data: Option<String>,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for YoutubeTrack {
    fn supports_seek(&self) -> bool {
        true
    }

    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let context    = serde_json::json!({ "visitorData": self.visitor_data });
        let mut last_error = String::from("No clients available");

        for client in &self.clients {
            let name = client.name().to_string();

            let url = match client
                .get_track_url(
                    &self.identifier,
                    &context,
                    self.cipher_manager.clone(),
                    self.oauth.clone(),
                )
                .await
            {
                Ok(Some(url)) => {
                    info!("YoutubeTrack: resolved '{}' using '{name}'", self.identifier);
                    url
                }
                Ok(None) => {
                    debug!("YoutubeTrack: client '{name}' returned no URL for '{}'", self.identifier);
                    continue;
                }
                Err(e) => {
                    warn!("YoutubeTrack: client '{name}' failed for '{}': {e}", self.identifier);
                    last_error = e.to_string();
                    if is_playability_error(&last_error) {
                        return Err(last_error);
                    }
                    continue;
                }
            };

            // URL mil gaya — hint nikalo, reader banao
            let is_hls     = url.contains(".m3u8") || url.contains("/playlist");
            let hint       = Some(detect_audio_kind(&url, is_hls));
            let proxy      = self.proxy.clone();
            let local_addr = self.local_addr;
            let cipher     = self.cipher_manager.clone();
            let url_clone  = url.clone();
            let name_clone = name.clone();

            match create_reader(&url_clone, &name_clone, local_addr, proxy, cipher).await {
                Ok(reader) => return Ok(ResolvedTrack::new(reader, hint)),
                Err(e) => {
                    warn!("YoutubeTrack: reader failed for '{name}': {e} — trying next client");
                    last_error = e.to_string();
                    continue;
                }
            }
        }

        error!("YoutubeTrack: all clients failed for '{}': {last_error}", self.identifier);
        Err(format!("All clients failed: {last_error}"))
    }
}


fn is_playability_error(msg: &str) -> bool {
    msg.contains("This video ")
        || msg.contains("This is a private video")
        || msg.contains("This trailer cannot be loaded")
}