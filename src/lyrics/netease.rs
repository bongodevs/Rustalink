use async_trait::async_trait;
use serde_json::Value;

use super::{LyricsProvider, utils};
use crate::protocol::{models::LyricsData, tracks::TrackInfo};

#[derive(Default)]
pub struct NeteaseProvider {
    client: reqwest::Client,
    cookies: String,
}

impl NeteaseProvider {
    pub fn new() -> Self {
        Self::default()
    }

    async fn search_track(&self, query: &str) -> Option<Value> {
        let url = format!(
            "https://music.163.com/api/search/pc?limit=10&type=1&offset=0&s={}",
            urlencoding::encode(query)
        );

        let resp = self.client.get(url)
            .header("Cookie", &self.cookies)
            .header("Referer", "https://music.163.com/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .send()
            .await.ok()?;

        let body: Value = resp.json().await.ok()?;
        let songs = body["result"]["songs"].as_array()?;

        songs.first().cloned()
    }
}

#[async_trait]
impl LyricsProvider for NeteaseProvider {
    fn name(&self) -> &'static str {
        "netease"
    }

    async fn load_lyrics(&self, track: &TrackInfo) -> Option<LyricsData> {
        let title = utils::clean_text(&track.title);
        let artist = utils::clean_text(&track.author);
        let query = format!("{} {}", title, artist);

        let song = self.search_track(&query).await?;
        let song_id = song["id"].as_i64()?;
        let song_name = song["name"].as_str().unwrap_or(&track.title).to_string();

        let lyrics_url = format!("https://music.163.com/api/song/lyric?id={}&lv=1", song_id);
        let resp = self.client.get(lyrics_url)
            .header("Cookie", &self.cookies)
            .header("Referer", "https://music.163.com/")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .send()
            .await.ok()?;

        let body: Value = resp.json().await.ok()?;
        let raw_lrc = body["lrc"]["lyric"].as_str()?;

        let lines = utils::parse_lrc(raw_lrc);
        if lines.is_empty() {
            return Some(LyricsData {
                name: song_name,
                author: artist,
                provider: "netease".to_string(),
                text: raw_lrc.to_string(),
                lines: None,
            });
        }

        let full_text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        Some(LyricsData {
            name: song_name,
            author: artist,
            provider: "netease".to_string(),
            text: full_text,
            lines: Some(lines),
        })
    }
}
