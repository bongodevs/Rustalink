use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{YouTubeClient, common::INNERTUBE_API};
use crate::{
    common::types::AnyResult,
    protocol::tracks::Track,
    sources::youtube::{
        cipher::YouTubeCipherManager, extractor::extract_from_player, oauth::YouTubeOAuth,
    },
};

const CLIENT_NAME: &str = "TVHTML5_UNPLUGGED";
const CLIENT_VERSION: &str = "6.13";
const USER_AGENT: &str = "Mozilla/5.0 (Linux armeabi-v7a; Android 7.1.2; Fire OS 6.0) Cobalt/22.lts.3.306369-gold (unlike Gecko) v8/8.8.278.8-jit gles Starboard/13, Amazon_ATV_mediatek8695_2019/NS6294 (Amazon, AFTMM, Wireless) com.amazon.firetv.youtube/22.3.r2.v66.0";

pub struct TvUnpluggedClient {
    http: Arc<reqwest::Client>,
    cipher_manager: Arc<YouTubeCipherManager>,
}

impl TvUnpluggedClient {
    pub fn new(http: Arc<reqwest::Client>, cipher_manager: Arc<YouTubeCipherManager>) -> Self {
        Self {
            http,
            cipher_manager,
        }
    }

    async fn player_request(
        &self,
        video_id: &str,
        visitor_data: Option<&str>,
        signature_timestamp: Option<u32>,
    ) -> AnyResult<Value> {
        let mut client_obj = json!({
            "clientName": CLIENT_NAME,
            "clientVersion": CLIENT_VERSION,
            "clientScreen": "EMBED"
        });

        if let Some(vd) = visitor_data {
            client_obj
                .as_object_mut()
                .unwrap()
                .insert("visitorData".to_string(), vd.into());
        }

        let mut body = json!({
            "context": {
                "client": client_obj,
                "thirdParty": { "embedUrl": "https://www.youtube.com/" }
            },
            "videoId": video_id,
            "params": "2AMB",
            "racyCheckOk": true,
            "contentCheckOk": true
        });

        let encrypted_host_flags = self.fetch_encrypted_host_flags(video_id).await;

        let mut playback_context = json!({
            "contentPlaybackContext": {}
        });

        if let Some(obj) = playback_context["contentPlaybackContext"].as_object_mut() {
            if let Some(st) = signature_timestamp {
                obj.insert("signatureTimestamp".to_string(), st.into());
            }
            if let Some(flags) = encrypted_host_flags {
                obj.insert("encryptedHostFlags".to_string(), flags.into());
            }
        }

        if !playback_context["contentPlaybackContext"]
            .as_object()
            .unwrap()
            .is_empty()
        {
            body.as_object_mut()
                .unwrap()
                .insert("playbackContext".to_string(), playback_context);
        }

        let url = format!("{}/youtubei/v1/player?prettyPrint=false", INNERTUBE_API);

        let mut req = self.http.post(&url).header("User-Agent", USER_AGENT);

        if let Some(vd) = visitor_data {
            req = req.header("X-Goog-Visitor-Id", vd);
        }

        let res = req.json(&body).send().await?;
        let status = res.status();
        let body_val: Value = res.json().await?;

        if !status.is_success() {
            tracing::error!(
                "TvUnplugged request failed (status={}): {}",
                status,
                body_val
            );
            return Err(format!("TvUnplugged player request failed (status={})", status).into());
        }

        Ok(body_val)
    }

    async fn fetch_encrypted_host_flags(&self, video_id: &str) -> Option<String> {
        let url = format!("https://www.youtube.com/embed/{}", video_id);
        let res = self
            .http
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send()
            .await
            .ok()?;

        let html = res.text().await.ok()?;
        let re = regex::Regex::new(r#""encryptedHostFlags":"([^"]+)""#).ok()?;
        re.captures(&html).map(|caps| caps[1].to_string())
    }
}

#[async_trait]
impl YouTubeClient for TvUnpluggedClient {
    fn name(&self) -> &str {
        "TvUnplugged"
    }

    fn client_name(&self) -> &str {
        CLIENT_NAME
    }

    fn client_version(&self) -> &str {
        CLIENT_VERSION
    }

    fn user_agent(&self) -> &str {
        USER_AGENT
    }

    fn can_handle_request(&self, identifier: &str) -> bool {
        if identifier.contains("list=") && !identifier.contains("list=RD") {
            return false;
        }
        true
    }

    async fn search(
        &self,
        _query: &str,
        _context: &Value,
        _oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Vec<Track>> {
        Err("TvUnplugged client does not support search".into())
    }

    async fn get_track_info(
        &self,
        track_id: &str,
        context: &Value,
        _oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<Track>> {
        let visitor_data = context
            .get("client")
            .and_then(|c| c.get("visitorData"))
            .and_then(|v| v.as_str())
            .or_else(|| context.get("visitorData").and_then(|v| v.as_str()));

        let signature_timestamp = self.cipher_manager.get_signature_timestamp().await.ok();
        let body = self
            .player_request(track_id, visitor_data, signature_timestamp)
            .await?;

        Ok(extract_from_player(&body, "youtube"))
    }

    async fn get_playlist(
        &self,
        _playlist_id: &str,
        _context: &Value,
        _oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<(Vec<Track>, String)>> {
        Ok(None)
    }

    async fn resolve_url(
        &self,
        _url: &str,
        _context: &Value,
        _oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<Track>> {
        Ok(None)
    }

    async fn get_track_url(
        &self,
        track_id: &str,
        context: &Value,
        _cipher_manager: Arc<YouTubeCipherManager>,
        _oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<String>> {
        let visitor_data = context
            .get("client")
            .and_then(|c| c.get("visitorData"))
            .and_then(|v| v.as_str())
            .or_else(|| context.get("visitorData").and_then(|v| v.as_str()));

        let body = self.player_request(track_id, visitor_data, None).await?;

        let streaming_data = match body.get("streamingData") {
            Some(sd) => sd,
            None => return Ok(None),
        };

        if let Some(hls) = streaming_data
            .get("hlsManifestUrl")
            .and_then(|v| v.as_str())
        {
            return Ok(Some(hls.to_string()));
        }

        let adaptive = streaming_data
            .get("adaptiveFormats")
            .and_then(|v| v.as_array());
        let formats = streaming_data.get("formats").and_then(|v| v.as_array());

        if let Some(best) =
            crate::sources::youtube::clients::common::select_best_audio_format(adaptive, formats)
        {
            return crate::sources::youtube::clients::common::resolve_format_url(
                best,
                &format!("https://www.youtube.com/watch?v={}", track_id),
                &self.cipher_manager,
            )
            .await;
        }

        Ok(None)
    }

    async fn get_player_body(
        &self,
        track_id: &str,
        visitor_data: Option<&str>,
        _oauth: Arc<YouTubeOAuth>,
    ) -> Option<serde_json::Value> {
        self.player_request(track_id, visitor_data, None).await.ok()
    }
}
