use std::net::IpAddr;

use async_trait::async_trait;
use base64::prelude::*;
use des::{
    Des,
    cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray},
};

use crate::{
    common::AudioFormat,
    config::HttpProxyConfig,
    sources::playable_track::{PlayableTrack, ResolvedTrack},
};

pub struct JioSaavnTrack {
    pub encrypted_url: String,
    pub secret_key: Vec<u8>,
    pub is_320: bool,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for JioSaavnTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let url = self.resolve_url().ok_or_else(|| {
            "Failed to decrypt JioSaavn URL. Check secretKey in config.toml".to_string()
        })?;

        let hint = format_hint_from_url(&url);
        let reader = super::reader::JioSaavnReader::new(&url, self.local_addr, self.proxy.clone())
            .await
            .map(|r| Box::new(r) as Box<dyn symphonia::core::io::MediaSource>)
            .map_err(|e| format!("Failed to open stream: {e}"))?;

        Ok(ResolvedTrack::new(reader, hint))
    }
}

impl JioSaavnTrack {
    fn resolve_url(&self) -> Option<String> {
        let mut url = self.decrypt_url(&self.encrypted_url)?;
        if self.is_320 {
            url = url.replace("_96.mp4", "_320.mp4");
        }
        Some(url)
    }

    fn decrypt_url(&self, encrypted: &str) -> Option<String> {
        if self.secret_key.len() != 8 {
            return None;
        }

        let cipher = Des::new_from_slice(&self.secret_key).ok()?;
        let mut data = BASE64_STANDARD.decode(encrypted).ok()?;

        for chunk in data.chunks_mut(8) {
            if chunk.len() == 8 {
                cipher.decrypt_block(GenericArray::from_mut_slice(chunk));
            }
        }

        if let Some(&last_byte) = data.last() {
            let padding = last_byte as usize;
            if (1..=8).contains(&padding) && data.len() >= padding {
                data.truncate(data.len() - padding);
            }
        }

        String::from_utf8(data).ok()
    }
}

fn format_hint_from_url(url: &str) -> Option<AudioFormat> {
    std::path::Path::new(url)
        .extension()
        .and_then(|s| s.to_str())
        .map(AudioFormat::from_ext)
        .filter(|f| *f != AudioFormat::Unknown)
        .or(Some(AudioFormat::Mp4))
}
