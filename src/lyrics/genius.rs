use std::sync::LazyLock;

use async_trait::async_trait;
use regex::Regex;

use super::{LyricsProvider, utils};
use crate::protocol::{
    models::{LyricsData, LyricsLine},
    tracks::TrackInfo,
};

static PRELOADED_STATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)window\.__PRELOADED_STATE__\s*=\s*JSON\.parse\('(.*?)'\);"#).expect("invalid PRELOADED_STATE_RE pattern")
});
static ESCAPE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\\(.)"#).expect("invalid ESCAPE_RE pattern"));
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<[^>]*>"#).expect("invalid TAG_RE pattern"));

#[derive(Default)]
pub struct GeniusProvider {
    client: reqwest::Client,
}

impl GeniusProvider {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl LyricsProvider for GeniusProvider {
    fn name(&self) -> &'static str {
        "genius"
    }

    async fn load_lyrics(&self, track: &TrackInfo) -> Option<LyricsData> {
        let title = utils::clean_text(&track.title);
        let author = utils::clean_text(&track.author);

        let query = if title.to_lowercase().starts_with(&author.to_lowercase()) {
            title.clone()
        } else {
            format!("{} {}", title, author)
        };

        let url = format!(
            "https://genius.com/api/search/multi?q={}",
            urlencoding::encode(&query)
        );
        let resp = self.client.get(url).send().await.ok()?;
        let search_data: serde_json::Value = resp.json().await.ok()?;

        let song = search_data["response"]["sections"]
            .as_array()?
            .iter()
            .find(|s| s["type"] == "song")?["hits"]
            .as_array()?
            .first()?["result"]
            .clone();

        let song_path = song["path"].as_str()?;
        let song_url = format!("https://genius.com{}", song_path);

        let song_resp = self.client.get(song_url).send().await.ok()?;
        let song_page = song_resp.text().await.ok()?;

        let re = &*PRELOADED_STATE_RE;
        let caps = re.captures(&song_page)?;
        let lyrics_data_raw = caps.get(1)?.as_str();

        let escape_re = &*ESCAPE_RE;
        let lyrics_data_unescaped = escape_re.replace_all(lyrics_data_raw, "$1");

        let lyrics_json: serde_json::Value = serde_json::from_str(&lyrics_data_unescaped).ok()?;

        let lyrics_content = lyrics_json["songPage"]["lyricsData"]["body"]["html"].as_str()?;

        let tag_re = &*TAG_RE;
        let lyrics_text = lyrics_content
            .replace("<br>", "\n")
            .replace("<br/>", "\n")
            .replace("<br />", "\n");
        let cleaned_lyrics = utils::unescape_html(&tag_re.replace_all(&lyrics_text, ""));

        let lines: Vec<LyricsLine> = cleaned_lyrics
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .filter(|l| !(l.starts_with('[') && l.ends_with(']')))
            .map(|l| LyricsLine {
                text: l.to_string(),
                timestamp: 0,
                duration: 0,
            })
            .collect();

        if lines.is_empty() {
            return None;
        }

        let full_text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        Some(LyricsData {
            name: song["title"].as_str().unwrap_or("original").to_string(),
            author: song["primary_artist"]["name"]
                .as_str()
                .unwrap_or(&track.author)
                .to_string(),
            provider: "genius".to_string(),
            text: full_text,
            lines: None,
        })
    }
}
