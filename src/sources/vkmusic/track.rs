use std::net::IpAddr;

use async_trait::async_trait;

use crate::{
    config::HttpProxyConfig,
    sources::{
        http::HttpTrack,
        playable_track::{PlayableTrack, ResolvedTrack},
    },
};

pub struct VkMusicTrack {
    pub stream_url: String,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for VkMusicTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        HttpTrack {
            url: self.stream_url.clone(),
            local_addr: self.local_addr,
            proxy: self.proxy.clone(),
        }
        .resolve()
        .await
    }
}
