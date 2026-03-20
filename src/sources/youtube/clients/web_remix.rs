use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};

use super::{YouTubeClient, core};
use crate::{
    common::types::AnyResult,
    protocol::tracks::{Track, TrackInfo},
    sources::youtube::{
        cipher::YouTubeCipherManager,
        clients::common::{ClientConfig, extract_thumbnail, is_duration, parse_duration},
        oauth::YouTubeOAuth,
    },
};

const CLIENT_NAME: &str = "WEB_REMIX";
const CLIENT_VERSION: &str = "1.20260121.03.00";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
     AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36";

const MUSIC_API: &str = "https://music.youtube.com";

pub struct WebRemixClient {
    http: Arc<reqwest::Client>,
}

impl WebRemixClient {
    pub fn new(http: Arc<reqwest::Client>) -> Self {
        Self { http }
    }

    fn config(&self) -> ClientConfig<'static> {
        ClientConfig {
            client_name: CLIENT_NAME,
            client_version: CLIENT_VERSION,
            client_id: "26",
            user_agent: USER_AGENT,
            ..Default::default()
        }
    }
}

#[async_trait]
impl YouTubeClient for WebRemixClient {
    fn name(&self) -> &str {
        "MusicWeb"
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
        let visitor_data = core::extract_visitor_data(context);

        let body = json!({
            "context": self.config().build_context(visitor_data),
            "query": query,
            "params": "EgWKAQIIAWoQEAMQBBAFEBAQCRAKEBUQEQ%3D%3D"
        });

        let url = format!("{}/youtubei/v1/search?prettyPrint=false", MUSIC_API);

        let mut req = self
            .http
            .post(&url)
            .header("User-Agent", USER_AGENT)
            .header("X-Goog-Api-Format-Version", "2")
            .header("Origin", MUSIC_API);

        if let Some(vd) = visitor_data {
            req = req.header("X-Goog-Visitor-Id", vd);
        }

        let req = req.json(&body);

        let res = req.send().await?;
        if !res.status().is_success() {
            return Err(format!("Music search failed: {}", res.status()).into());
        }

        let response: Value = res.json().await?;
        let mut tracks = Vec::new();

        // Improved navigation for YouTube Music search results
        let tab_content = response
            .get("contents")
            .and_then(|c| c.get("tabbedSearchResultsRenderer"))
            .and_then(|t| t.get("tabs"))
            .and_then(|t| t.get(0))
            .and_then(|t| t.get("tabRenderer"))
            .and_then(|t| t.get("content"));

        let mut shelf_contents = None;

        fn find_shelf(content: &Value) -> Option<&Vec<Value>> {
            if let Some(section_list) = content.get("sectionListRenderer")
                && let Some(sections) = section_list.get("contents").and_then(|c| c.as_array())
            {
                for section in sections {
                    if let Some(shelf) = section.get("musicShelfRenderer")
                        && let Some(items) = shelf.get("contents").and_then(|c| c.as_array())
                    {
                        return Some(items);
                    }
                }
            }
            None
        }

        if let Some(tab) = tab_content {
            shelf_contents = find_shelf(tab);

            if shelf_contents.is_none()
                && let Some(split_view) = tab.get("musicSplitViewRenderer")
                && let Some(main_content) = split_view.get("mainContent")
            {
                shelf_contents = find_shelf(main_content);
            }
        }

        if let Some(items) = shelf_contents {
            for item in items {
                let renderer = item
                    .get("musicResponsiveListItemRenderer")
                    .or_else(|| item.get("musicTwoColumnItemRenderer"));

                if let Some(renderer) = renderer {
                    // Extract video ID safely
                    let id = renderer
                        .get("playlistItemData")
                        .and_then(|d| d.get("videoId"))
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            renderer
                                .get("doubleTapCommand")
                                .and_then(|c| c.get("watchEndpoint"))
                                .and_then(|w| w.get("videoId"))
                                .and_then(|v| v.as_str())
                        })
                        .or_else(|| renderer.get("videoId").and_then(|v| v.as_str()));

                    if let Some(id) = id {
                        // Improved title extraction
                        let title = renderer
                            .get("flexColumns")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("musicResponsiveListItemFlexColumnRenderer"))
                            .and_then(|r| r.get("text"))
                            .and_then(|t| t.get("runs"))
                            .and_then(|r| r.get(0))
                            .and_then(|r| r.get("text"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("Unknown Title");

                        // Improved author and length extraction from runs
                        let mut author = "Unknown Artist".to_string();
                        let mut length_ms = 0u64;

                        // Check flex columns for author and duration
                        if let Some(flex_cols) =
                            renderer.get("flexColumns").and_then(|c| c.as_array())
                        {
                            // Column 1 is usually Artist
                            if flex_cols.len() > 1
                                && let Some(a) = flex_cols[1]
                                    .get("musicResponsiveListItemFlexColumnRenderer")
                                    .and_then(|r| r.get("text"))
                                    .and_then(|t| t.get("runs"))
                                    .and_then(|r| r.get(0))
                                    .and_then(|r| r.get("text"))
                                    .and_then(|t| t.as_str())
                            {
                                author = a.to_string();
                            }

                            // Scan all columns for duration string
                            for col in flex_cols {
                                if let Some(runs) = col
                                    .get("musicResponsiveListItemFlexColumnRenderer")
                                    .and_then(|r| r.get("text"))
                                    .and_then(|t| t.get("runs"))
                                    .and_then(|r| r.as_array())
                                {
                                    for run in runs {
                                        if let Some(text) = run.get("text").and_then(|t| t.as_str())
                                            && is_duration(text)
                                        {
                                            length_ms = parse_duration(text);
                                            break;
                                        }
                                    }
                                }
                                if length_ms > 0 {
                                    break;
                                }
                            }
                        }

                        // Fallback author if still default
                        if author == "Unknown Artist"
                            && let Some(subtitle_runs) = renderer
                                .get("subtitle")
                                .and_then(|s| s.get("runs"))
                                .and_then(|r| r.as_array())
                            && !subtitle_runs.is_empty()
                            && let Some(a) = subtitle_runs[0].get("text").and_then(|t| t.as_str())
                        {
                            author = a.to_string();
                        }

                        // Artwork URL extraction
                        let artwork_url = extract_thumbnail(renderer, Some(id));

                        let info = TrackInfo {
                            identifier: id.to_string(),
                            is_seekable: true,
                            title: title.to_string(),
                            author,
                            length: length_ms,
                            is_stream: false,
                            uri: Some(format!("https://music.youtube.com/watch?v={}", id)),
                            source_name: "youtube".to_string(),
                            isrc: None,
                            artwork_url,
                            position: 0,
                        };
                        tracks.push(Track::new(info));
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
        oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<Track>> {
        core::standard_get_track_info(
            self,
            core::StandardPlayerOptions {
                http: &self.http,
                track_id,
                context,
                oauth,
                signature_timestamp: None,
                encrypted_host_flags: None,
                config_builder: || self.config(),
            },
        )
        .await
    }

    async fn get_playlist(
        &self,
        playlist_id: &str,
        context: &Value,
        oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<(Vec<Track>, String)>> {
        let visitor_data = core::extract_visitor_data(context);

        let is_mix_playlist = playlist_id.starts_with("RDCLAK5uy_")
            || playlist_id.starts_with("RDAMVM")
            || playlist_id.starts_with("RDMM")
            || playlist_id.starts_with("RD");

        if is_mix_playlist {
            let next_body = json!({
                "context": self.config().build_context(visitor_data),
                "playlistId": playlist_id,
                "enablePersistentPlaylistPanel": true,
                "isAudioOnly": true
            });

            let next_url = format!("{}/youtubei/v1/next?prettyPrint=false", MUSIC_API);

            let mut next_req = self
                .http
                .post(&next_url)
                .header("User-Agent", USER_AGENT)
                .header("X-YouTube-Client-Name", "26")
                .header("X-YouTube-Client-Version", CLIENT_VERSION);

            if let Some(vd) = visitor_data {
                next_req = next_req.header("X-Goog-Visitor-Id", vd);
            }

            if let Ok(res) = next_req.json(&next_body).send().await {
                if res.status().is_success() {
                    let body: Value = res.json().await?;
                    if let Some(result) =
                        crate::sources::youtube::extractor::extract_from_next(&body, "youtube")
                    {
                        return Ok(Some(result));
                    }
                    tracing::debug!(
                        "WebRemix: /next endpoint returned but extraction failed for playlist {}",
                        playlist_id
                    );
                }
            }
        }

        core::standard_get_playlist(self, &self.http, playlist_id, context, oauth, || {
            self.config()
        })
        .await
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
        cipher_manager: Arc<YouTubeCipherManager>,
        oauth: Arc<YouTubeOAuth>,
    ) -> AnyResult<Option<String>> {
        let signature_timestamp = cipher_manager.get_signature_timestamp().await.ok();
        core::standard_get_track_url(
            self,
            core::StandardUrlOptions {
                http: &self.http,
                track_id,
                context,
                cipher_manager,
                oauth,
                signature_timestamp,
                encrypted_host_flags: None,
                config_builder: || self.config(),
            },
        )
        .await
    }

    async fn get_player_body(
        &self,
        track_id: &str,
        visitor_data: Option<&str>,
        _oauth: Arc<YouTubeOAuth>,
    ) -> Option<serde_json::Value> {
        crate::sources::youtube::clients::common::make_player_request(
            crate::sources::youtube::clients::common::PlayerRequestOptions {
                http: &self.http,
                config: &self.config(),
                video_id: track_id,
                params: None,
                visitor_data,
                signature_timestamp: None,
                auth_header: None,
                referer: None,
                origin: Some(MUSIC_API),
                po_token: None,
                encrypted_host_flags: None,
                attestation_request: None,
                serialized_third_party_embed_config: false,
            },
        )
        .await
        .ok()
    }
}
