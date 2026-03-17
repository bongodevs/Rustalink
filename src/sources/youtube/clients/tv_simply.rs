use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{YouTubeClient, common::INNERTUBE_API};
use crate::{
    common::types::AnyResult,
    protocol::tracks::Track,
    sources::youtube::{
        cipher::YouTubeCipherManager,
        clients::common::ClientConfig,
        extractor::{extract_from_next, extract_from_player, extract_track, find_section_list},
        oauth::YouTubeOAuth,
    },
};

const CLIENT_NAME: &str = "TVHTML5_SIMPLY";
const CLIENT_VERSION: &str = "1.0";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36";

pub struct TvSimplyClient {
    http: Arc<reqwest::Client>,
    cipher_manager: Arc<YouTubeCipherManager>,
}

impl TvSimplyClient {
    pub fn new(http: Arc<reqwest::Client>, cipher_manager: Arc<YouTubeCipherManager>) -> Self {
        Self {
            http,
            cipher_manager,
        }
    }

    fn config(&self) -> ClientConfig<'_> {
        ClientConfig {
            client_name: CLIENT_NAME,
            client_version: CLIENT_VERSION,
            client_id: "TVHTML5_SIMPLY",
            user_agent: USER_AGENT,
            attestation_request: Some(json!({ "omitBotguardData": true })),
            ..Default::default()
        }
    }

    async fn player_request(
        &self,
        video_id: &str,
        visitor_data: Option<&str>,
        signature_timestamp: Option<u32>,
    ) -> AnyResult<Value> {
        let encrypted_host_flags = self.fetch_encrypted_host_flags(video_id).await;

        crate::sources::youtube::clients::common::make_player_request(
            crate::sources::youtube::clients::common::PlayerRequestOptions {
                http: &self.http,
                config: &self.config(),
                video_id,
                params: Some("2AMB"),
                visitor_data,
                signature_timestamp,
                auth_header: None,
                referer: None,
                origin: Some("https://www.youtube.com"),
                po_token: None,
                encrypted_host_flags,
                attestation_request: Some(json!({ "omitBotguardData": true })),
                serialized_third_party_embed_config: false,
            },
        )
        .await
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
impl YouTubeClient for TvSimplyClient {
    fn name(&self) -> &str {
        "TvSimply"
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

    async fn search(
        &self,
        query: &str,
        context: &Value,
        _oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Vec<Track>> {
        let visitor_data = context
            .get("client")
            .and_then(|c| c.get("visitorData"))
            .and_then(|v| v.as_str())
            .or_else(|| context.get("visitorData").and_then(|v| v.as_str()));

        let body = json!({
            "context": self.config().build_context(visitor_data),
            "query": query,
            "params": "EgIQAfABAQ=="
        });

        let url = format!("{}/youtubei/v1/search?prettyPrint=false", INNERTUBE_API);

        let res = self
            .http
            .post(&url)
            .header("User-Agent", USER_AGENT)
            .header("X-YouTube-Client-Name", "TVHTML5_SIMPLY")
            .header("X-YouTube-Client-Version", CLIENT_VERSION)
            .header("X-Goog-Api-Format-Version", "2")
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            return Err(format!("TvSimply search failed: {}", res.status()).into());
        }

        let response: Value = res.json().await?;
        let mut tracks = Vec::new();

        if let Some(section_list) = find_section_list(&response)
            && let Some(contents) = section_list.get("contents").and_then(|c| c.as_array())
        {
            for section in contents {
                if let Some(items) = section
                    .get("itemSectionRenderer")
                    .and_then(|i| i.get("contents"))
                    .and_then(|c| c.as_array())
                {
                    for item in items {
                        if let Some(track) = extract_track(item, "youtube") {
                            tracks.push(track);
                        }
                    }
                }
            }
        }

        Ok(tracks)
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
        playlist_id: &str,
        context: &Value,
        _oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<(Vec<Track>, String)>> {
        let visitor_data = context
            .get("client")
            .and_then(|c| c.get("visitorData"))
            .and_then(|v| v.as_str())
            .or_else(|| context.get("visitorData").and_then(|v| v.as_str()));

        let body = json!({
            "context": self.config().build_context(visitor_data),
            "playlistId": playlist_id,
        });

        let url = format!("{}/youtubei/v1/next?prettyPrint=false", INNERTUBE_API);

        let res = self
            .http
            .post(&url)
            .header("User-Agent", USER_AGENT)
            .header("X-YouTube-Client-Name", "TVHTML5_SIMPLY")
            .header("X-YouTube-Client-Version", CLIENT_VERSION)
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            return Ok(None);
        }

        let response: Value = res.json().await?;
        Ok(extract_from_next(&response, "youtube"))
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
        _track_id: &str,
        _context: &Value,
        _cipher_manager: Arc<YouTubeCipherManager>,
        _oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<String>> {
        // let visitor_data = context
        //     .get("client")
        //     .and_then(|c| c.get("visitorData"))
        //     .and_then(|v| v.as_str())
        //     .or_else(|| context.get("visitorData").and_then(|v| v.as_str()));
        //
        // let body = self.player_request(track_id, visitor_data, None).await?;
        //
        // let streaming_data = match body.get("streamingData") {
        //     Some(sd) => sd,
        //     None => {
        //         tracing::warn!(
        //             "TvSimply: No streamingData found for {}. Playability Status: {:?}",
        //             track_id,
        //             body.get("playabilityStatus")
        //         );
        //         return Ok(None);
        //     }
        // };
        //
        // if let Some(hls) = streaming_data
        //     .get("hlsManifestUrl")
        //     .and_then(|v| v.as_str())
        // {
        //     return Ok(Some(hls.to_string()));
        // }
        //
        // let adaptive = streaming_data
        //     .get("adaptiveFormats")
        //     .and_then(|v| v.as_array());
        // let formats = streaming_data.get("formats").and_then(|v| v.as_array());
        //
        // if let Some(best) =
        //     crate::sources::youtube::clients::common::select_best_audio_format(adaptive, formats)
        // {
        //     return crate::sources::youtube::clients::common::resolve_format_url(
        //         best,
        //         &format!("https://www.youtube.com/watch?v={}", track_id),
        //         &self.cipher_manager,
        //     )
        //     .await;
        // }
        //
        // tracing::warn!(
        //     "TvSimply: No suitable audio format found for {}. formats: {:?}",
        //     track_id,
        //     streaming_data.get("formats")
        // );

        tracing::debug!("{} client does not provide direct track URLs", self.name());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::sources::YouTubeCipherConfig, sources::youtube::cipher::YouTubeCipherManager,
    };

    #[tokio::test]
    async fn test_search() {
        let http = Arc::new(reqwest::Client::new());
        let cipher = Arc::new(YouTubeCipherManager::new(YouTubeCipherConfig::default()));
        let client = TvSimplyClient::new(http, cipher);
        let oauth = Arc::new(YouTubeOAuth::new(vec![]));

        let result = client.search("test", &json!({}), oauth).await.unwrap();
        assert!(!result.is_empty(), "Search should return tracks");
    }

    #[tokio::test]
    async fn test_playlist() {
        let http = Arc::new(reqwest::Client::new());
        let cipher = Arc::new(YouTubeCipherManager::new(YouTubeCipherConfig::default()));
        let client = TvSimplyClient::new(http, cipher);
        let oauth = Arc::new(YouTubeOAuth::new(vec![]));

        // Use a known playlist ID
        let result = client
            .get_playlist("PLFsQleAWXsj_4yDeebiIADdH5FMayBiJo", &json!({}), oauth)
            .await
            .unwrap();
        assert!(result.is_some(), "Playlist should return tracks");
        assert!(
            !result.unwrap().0.is_empty(),
            "Playlist should not be empty"
        );
    }
}

#[cfg(test)]
mod get_track_tests {
    use super::*;
    use crate::{
        config::sources::YouTubeCipherConfig, sources::youtube::cipher::YouTubeCipherManager,
    };

    #[tokio::test]
    async fn test_get_track_url() {
        let http = Arc::new(reqwest::Client::new());
        let cipher = Arc::new(YouTubeCipherManager::new(YouTubeCipherConfig::default()));
        let client = TvSimplyClient::new(http, cipher.clone());
        //let oauth = Arc::new(YouTubeOAuth::new(vec![]));

        let body = client
            .player_request("3Z_x7vBqr6E", None, None)
            .await
            .unwrap();
        println!("Body: {}", serde_json::to_string_pretty(&body).unwrap());
    }
}
