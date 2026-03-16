use std::net::IpAddr;

use async_trait::async_trait;

use crate::{
    common::types::AudioFormat,
    config::HttpProxyConfig,
    sources::playable_track::{PlayableTrack, ResolvedTrack},
};

#[derive(Debug, Clone)]
pub enum SoundCloudStreamKind {
    ProgressiveMp3,
    ProgressiveAac,
    HlsOpus,
    HlsMp3,
    HlsAac,
}

pub struct SoundCloudTrack {
    pub stream_url: String,
    pub kind: SoundCloudStreamKind,
    pub bitrate_bps: u64,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for SoundCloudTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let (reader, hint) = match self.kind {
            SoundCloudStreamKind::ProgressiveMp3 => (
                super::reader::SoundCloudReader::new(
                    &self.stream_url,
                    self.local_addr,
                    self.proxy.clone(),
                )
                .await
                .map(|r| Box::new(r) as Box<dyn symphonia::core::io::MediaSource>)
                .map_err(|e| format!("Failed to open stream: {e}"))?,
                AudioFormat::Mp3,
            ),

            SoundCloudStreamKind::ProgressiveAac => (
                super::reader::SoundCloudReader::new(
                    &self.stream_url,
                    self.local_addr,
                    self.proxy.clone(),
                )
                .await
                .map(|r| Box::new(r) as Box<dyn symphonia::core::io::MediaSource>)
                .map_err(|e| format!("Failed to open stream: {e}"))?,
                AudioFormat::Mp4,
            ),

            SoundCloudStreamKind::HlsOpus => (
                super::reader::SoundCloudHlsReader::new(
                    &self.stream_url,
                    self.bitrate_bps,
                    self.local_addr,
                    self.proxy.clone(),
                )
                .await
                .map(|r| Box::new(r) as Box<dyn symphonia::core::io::MediaSource>)
                .map_err(|e| format!("Failed to init HLS reader: {e}"))?,
                AudioFormat::Opus,
            ),

            SoundCloudStreamKind::HlsMp3 => (
                super::reader::SoundCloudHlsReader::new(
                    &self.stream_url,
                    self.bitrate_bps,
                    self.local_addr,
                    self.proxy.clone(),
                )
                .await
                .map(|r| Box::new(r) as Box<dyn symphonia::core::io::MediaSource>)
                .map_err(|e| format!("Failed to init HLS reader: {e}"))?,
                AudioFormat::Mp3,
            ),

            SoundCloudStreamKind::HlsAac => (
                super::reader::SoundCloudHlsReader::new(
                    &self.stream_url,
                    self.bitrate_bps,
                    self.local_addr,
                    self.proxy.clone(),
                )
                .await
                .map(|r| Box::new(r) as Box<dyn symphonia::core::io::MediaSource>)
                .map_err(|e| format!("Failed to init HLS reader: {e}"))?,
                AudioFormat::Aac,
            ),
        };

        Ok(ResolvedTrack::new(reader, Some(hint)))
    }
}
