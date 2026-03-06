use async_trait::async_trait;

use super::{LyricsProvider, utils};
use crate::protocol::{
    models::{LyricsData, LyricsLine},
    tracks::TrackInfo,
};

#[derive(Default)]
pub struct LrcLibProvider {
    client: reqwest::Client,
}

impl LrcLibProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl LyricsProvider for LrcLibProvider {
    fn name(&self) -> &'static str {
        "lrclib"
    }

    async fn load_lyrics(&self, track: &TrackInfo) -> Option<LyricsData> {
        let title = utils::clean_text(&track.title);
        let author = utils::clean_text(&track.author);

        let query = format!("{} {}", title, author);
        let url = format!(
            "https://lrclib.net/api/search?q={}",
            urlencoding::encode(&query)
        );

        let resp = self.client.get(url).send().await.ok()?;
        let results: serde_json::Value = resp.json().await.ok()?;

        let results_arr = results.as_array()?;
        if results_arr.is_empty() {
            return None;
        }

        let title_lower = title.to_lowercase();
        let author_lower = author.to_lowercase();

        let best_match = results_arr
            .iter()
            .find(|r| {
                let r_title =
                    utils::clean_text(r["trackName"].as_str().unwrap_or("")).to_lowercase();
                let r_author =
                    utils::clean_text(r["artistName"].as_str().unwrap_or("")).to_lowercase();
                let instrumental = r["instrumental"].as_bool().unwrap_or(false);

                r_title == title_lower && r_author == author_lower && !instrumental
            })
            .or_else(|| {
                results_arr.iter().find(|r| {
                    let r_title =
                        utils::clean_text(r["trackName"].as_str().unwrap_or("")).to_lowercase();
                    let instrumental = r["instrumental"].as_bool().unwrap_or(false);
                    r_title == title_lower && !instrumental
                })
            })
            .or_else(|| {
                results_arr
                    .iter()
                    .find(|r| !r["instrumental"].as_bool().unwrap_or(false))
            })?;

        let mut lines = Vec::new();
        let mut synced = false;

        if let Some(synced_lyrics) = best_match["syncedLyrics"].as_str() {
            lines = utils::parse_lrc(synced_lyrics);
            synced = true;
        } else if let Some(plain_lyrics) = best_match["plainLyrics"].as_str() {
            lines = plain_lyrics
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .map(|text| LyricsLine {
                    text: text.to_string(),
                    timestamp: 0,
                    duration: 0,
                })
                .collect();
        }

        if lines.is_empty() {
            return None;
        }

        let full_text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        Some(LyricsData {
            name: best_match["trackName"]
                .as_str()
                .unwrap_or(&track.title)
                .to_string(),
            author: best_match["artistName"]
                .as_str()
                .unwrap_or(&track.author)
                .to_string(),
            provider: "lrclib".to_string(),
            text: full_text,
            lines: if synced { Some(lines) } else { None },
        })
    }
}
