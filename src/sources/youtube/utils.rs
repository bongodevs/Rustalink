use std::sync::Arc;

use symphonia::core::io::MediaSource;

use crate::{
    common::types::AudioFormat,
    sources::{
        http::reader::HttpReader,
        youtube::{cipher::YouTubeCipherManager, hls::HlsReader, reader::YoutubeReader},
    },
};

pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36";

pub fn detect_audio_kind(url: &str, is_hls: bool) -> AudioFormat {
    if is_hls {
        AudioFormat::Aac
    } else {
        AudioFormat::from_url(url)
    }
}

pub async fn create_reader(
    url: &str,
    client_name: &str,
    local_addr: Option<std::net::IpAddr>,
    proxy: Option<crate::config::HttpProxyConfig>,
    cipher_manager: Arc<YouTubeCipherManager>,
) -> AnyResult<Box<dyn MediaSource>> {
    if url.contains(".m3u8") || url.contains("/playlist") {
        Ok(Box::new(
            HlsReader::new(url, local_addr, Some(cipher_manager), None, proxy).await?,
        ))
    } else if client_name == "TV" {
        Ok(Box::new(YoutubeReader::new(url, local_addr, proxy).await?))
    } else {
        Ok(Box::new(HttpReader::new(url, local_addr, proxy).await?))
    }
}
type AnyResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn parse_playability_status(body: &serde_json::Value) -> Result<(), String> {
    let playability = body
        .get("playabilityStatus")
        .and_then(|p| p.get("status"))
        .and_then(|s| s.as_str())
        .unwrap_or("UNKNOWN");

    if playability == "OK" {
        return Ok(());
    }

    let p = body.get("playabilityStatus");
    let reason = p
        .and_then(|p| p.get("reason"))
        .and_then(|r| r.as_str())
        .unwrap_or("unknown reason");

    match playability {
        "ERROR" => Err(reason.to_string()),
        "UNPLAYABLE" => {
            if reason == "unknown reason" {
                Err("This video is unplayable.".to_string())
            } else {
                Err(reason.to_string())
            }
        }
        "LOGIN_REQUIRED" => {
            if reason.contains("This video is private") {
                Err("This is a private video.".to_string())
            } else if reason.contains("This video may be inappropriate for some users") {
                Err("This video requires age verification.".to_string())
            } else {
                Err("This video requires login.".to_string())
            }
        }
        "CONTENT_CHECK_REQUIRED" => Err(reason.to_string()),
        "LIVE_STREAM_OFFLINE" => {
            if let Some(err_screen) = p.and_then(|p| p.get("errorScreen"))
                && err_screen.get("ypcTrailerRenderer").is_some()
            {
                return Err("This trailer cannot be loaded.".to_string());
            }
            Err(reason.to_string())
        }
        _ => Err("This video cannot be viewed anonymously.".to_string()),
    }
}
