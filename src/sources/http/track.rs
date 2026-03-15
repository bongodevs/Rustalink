use async_trait::async_trait;

use crate::{
    common::types::AudioFormat,
    config::HttpProxyConfig,
    sources::{
        http::reader,
        playable_track::{PlayableTrack, ResolvedTrack},
    },
};

pub struct HttpTrack {
    pub url: String,
    pub local_addr: Option<std::net::IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for HttpTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let hint = std::path::Path::new(&self.url)
            .extension()
            .and_then(|s| s.to_str())
            .map(AudioFormat::from_ext)
            .filter(|f| *f != AudioFormat::Unknown);

        let reader = reader::HttpReader::new(&self.url, self.local_addr, self.proxy.clone()).await
            .map(|r| Box::new(r) as Box<dyn symphonia::core::io::MediaSource>)
            .map_err(|e| format!("Failed to open stream: {e}"))?;

        Ok(ResolvedTrack::new(reader, hint))
    }
}
