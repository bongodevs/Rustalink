use async_trait::async_trait;
use serde_json::Value;

use super::{
    LyricsProvider,
    utils::{self, TokenManager},
};
use crate::protocol::{
    models::{LyricsData, LyricsLine},
    tracks::TrackInfo,
};

const APP_ID: &str = "web-desktop-app-v1.0";
const TOKEN_TTL: u64 = 55_000;
const DEFAULT_COOKIE: &str =
    "AWSELB=unknown; x-mxm-user-id=undefined; x-mxm-token-guid=undefined; mxm-encrypted-token=";

#[derive(Default)]
pub struct MusixmatchProvider {
    client: reqwest::Client,
    token_manager: TokenManager,
    guid: String,
}

impl MusixmatchProvider {
    pub fn new() -> Self {
        Self {
            token_manager: TokenManager::new(TOKEN_TTL),
            ..Self::default()
        }
    }

    async fn get_token(&self) -> Option<String> {
        self.token_manager
            .get_token(|| async {
                let resp = self
                    .client
                    .get("https://apic-desktop.musixmatch.com/ws/1.1/token.get")
                    .query(&[("app_id", APP_ID)])
                    .header("Cookie", DEFAULT_COOKIE)
                    .send()
                    .await
                    .ok()?;

                let body: Value = resp.json().await.ok()?;
                body.get("message")?
                    .get("body")?
                    .get("user_token")?
                    .as_str()
                    .map(|s| s.to_string())
            })
            .await
    }

    fn parse_subtitles(&self, sub_json_str: &str) -> Option<Vec<LyricsLine>> {
        let sub_data: Value = serde_json::from_str(sub_json_str).ok()?;
        let arr = sub_data.as_array()?;
        let mut lines = Vec::new();
        for item in arr {
            let text = item
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let time_data = item.get("time")?;
            let total = time_data
                .get("total")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let duration = time_data
                .get("duration")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            lines.push(LyricsLine {
                text,
                timestamp: (total * 1000.0) as u64,
                duration: (duration * 1000.0) as u64,
            });
        }
        Some(lines)
    }
}

#[async_trait]
impl LyricsProvider for MusixmatchProvider {
    fn name(&self) -> &'static str {
        "musixmatch"
    }

    async fn load_lyrics(&self, track: &TrackInfo) -> Option<LyricsData> {
        let token = self.get_token().await?;
        let title = utils::clean_text(&track.title);
        let artist = &track.author;

        let query_params = [
            ("format", "json"),
            ("namespace", "lyrics_richsynched"),
            ("subtitle_format", "mxm"),
            ("q_track", &title),
            ("q_artist", artist),
            ("usertoken", &token),
            ("app_id", APP_ID),
            ("guid", &self.guid),
        ];

        let resp = self
            .client
            .get("https://apic-desktop.musixmatch.com/ws/1.1/macro.subtitles.get")
            .query(&query_params)
            .send()
            .await
            .ok()?;

        let body: Value = resp.json().await.ok()?;
        let message = body.get("message")?;

        if message.get("header")?.get("status_code")?.as_i64() != Some(200) {
            // Try title only fallback search if artist search failed
            let resp = self
                .client
                .get("https://apic-desktop.musixmatch.com/ws/1.1/track.search")
                .query(&[
                    ("format", "json"),
                    ("q", &format!("{} {}", title, artist)),
                    ("f_has_lyrics", "1"),
                    ("s_track_rating", "desc"),
                    ("usertoken", &token),
                    ("app_id", APP_ID),
                ])
                .send()
                .await
                .ok()?;

            let body: Value = resp.json().await.ok()?;
            let track_list = body
                .get("message")?
                .get("body")?
                .get("track_list")?
                .as_array()?;
            if track_list.is_empty() {
                return None;
            }

            let track_id = track_list[0].get("track")?.get("track_id")?.as_i64()?;

            // Fetch subtitles individually
            let sub_resp = self
                .client
                .get("https://apic-desktop.musixmatch.com/ws/1.1/track.subtitle.get")
                .query(&[
                    ("format", "json"),
                    ("track_id", &track_id.to_string()),
                    ("subtitle_format", "mxm"),
                    ("usertoken", &token),
                    ("app_id", APP_ID),
                ])
                .send()
                .await
                .ok()?;

            let sub_body: Value = sub_resp.json().await.ok()?;
            let sub_msg = sub_body.get("message")?;
            if sub_msg.get("header")?.get("status_code")?.as_i64() != Some(200) {
                // Try plain lyrics
                let lyr_resp = self
                    .client
                    .get("https://apic-desktop.musixmatch.com/ws/1.1/track.lyrics.get")
                    .query(&[
                        ("format", "json"),
                        ("track_id", &track_id.to_string()),
                        ("usertoken", &token),
                        ("app_id", APP_ID),
                    ])
                    .send()
                    .await
                    .ok()?;
                let lyr_body: Value = lyr_resp.json().await.ok()?;
                let lyr_text = lyr_body
                    .get("message")?
                    .get("body")?
                    .get("lyrics")?
                    .get("lyrics_body")?
                    .as_str()?;

                return Some(LyricsData {
                    name: title,
                    author: artist.clone(),
                    provider: "musixmatch".to_string(),
                    text: lyr_text.to_string(),
                    lines: None,
                });
            }

            let sub_text = sub_msg
                .get("body")?
                .get("subtitle")?
                .get("subtitle_body")?
                .as_str()?;
            let lines = self.parse_subtitles(sub_text)?;

            return Some(LyricsData {
                name: title,
                author: artist.clone(),
                provider: "musixmatch".to_string(),
                text: lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                lines: Some(lines),
            });
        }

        let calls = message.get("body")?.get("macro_calls")?;

        let lyrics_body = calls
            .get("track.lyrics.get")?
            .get("message")?
            .get("body")?
            .get("lyrics")?
            .get("lyrics_body")?
            .as_str()?;

        let subtitles_body = message
            .pointer("/body/macro_calls/track.subtitles.get/message/body/subtitle_list")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|f| f.pointer("/subtitle/subtitle_body"))
            .and_then(|b| b.as_str());

        let mut lines = Vec::new();
        let mut synced = false;

        if let Some(sub_json_str) = subtitles_body
            && let Some(parsed_lines) = self.parse_subtitles(sub_json_str)
        {
            lines = parsed_lines;
            synced = true;
        }

        if !synced {
            lines = lyrics_body
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| LyricsLine {
                    text: l.to_string(),
                    timestamp: 0,
                    duration: 0,
                })
                .collect();
        }

        Some(LyricsData {
            name: title,
            author: artist.clone(),
            provider: "musixmatch".to_string(),
            text: lyrics_body.to_string(),
            lines: if synced { Some(lines) } else { None },
        })
    }
}
