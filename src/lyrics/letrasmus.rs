use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;

use super::{LyricsProvider, utils};
use crate::protocol::{
    models::{LyricsData, LyricsLine},
    tracks::TrackInfo,
};

pub struct LetrasMusProvider {
    client: reqwest::Client,
}

impl Default for LetrasMusProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LetrasMusProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LyricsProvider for LetrasMusProvider {
    fn name(&self) -> &'static str {
        "letrasmus"
    }

    async fn load_lyrics(&self, track: &TrackInfo) -> Option<LyricsData> {
        let title = utils::clean_text(&track.title);
        let author = utils::clean_text(&track.author);
        let query = format!("{} {}", title, author);

        // Search via Solr API
        let search_url = format!(
            "https://solr.sscdn.co/letras/m1/?q={}&wt=json",
            urlencoding::encode(&query)
        );
        let resp = self.client.get(search_url).send().await.ok()?;
        let search_data: Value = resp.json().await.ok()?;

        let docs = search_data["response"]["docs"].as_array()?;
        let best = docs
            .iter()
            .find(|d| d["t"] == "2" && d["dns"].is_string() && d["url"].is_string())?;

        let dns = best["dns"].as_str()?;
        let url_path = best["url"].as_str()?;
        let page_url = format!("https://www.letras.mus.br/{}/{}/", dns, url_path);

        // Fetch HTML to extract metadata and translation links
        let html = self
            .client
            .get(&page_url)
            .send()
            .await
            .ok()?
            .text()
            .await
            .ok()?;

        // Extract _omq data (metadata)
        let omq_re = Regex::new(r#"_omq\.push\(\['ui/lyric',\s*(\{[\s\S]*?\})\s*,"#).unwrap();
        let omq = omq_re
            .captures(&html)
            .and_then(|c| serde_json::from_str::<Value>(c.get(1)?.as_str()).ok());

        let letras_id = omq.as_ref().and_then(|o| o["ID"].as_i64());
        let youtube_id = omq.as_ref().and_then(|o| o["YoutubeID"].as_str());

        // Try synced lyrics if available
        if let (Some(l_id), Some(y_id)) = (letras_id, youtube_id) {
            let api_url = format!(
                "https://www.letras.mus.br/api/v2/subtitle/{}/{}/",
                l_id, y_id
            );
            if let Ok(api_resp) = self.client.get(api_url).send().await
                && let Ok(api_data) = api_resp.json::<Value>().await
            {
                let sub_val = api_data["Original"]["Subtitle"]
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok());
                if let Some(sub_arr) = sub_val.as_ref().and_then(|v| v.as_array()) {
                    let lines: Vec<LyricsLine> = sub_arr
                        .iter()
                        .filter_map(|e| {
                            let arr = e.as_array()?;
                            let text = arr.first()?.as_str()?;
                            let start = arr.get(1)?.as_f64()?;
                            let end = arr.get(2)?.as_f64()?;
                            Some(LyricsLine {
                                text: text.to_string(),
                                timestamp: (start * 1000.0) as u64,
                                duration: ((end - start) * 1000.0) as u64,
                            })
                        })
                        .collect();

                    if !lines.is_empty() {
                        return Some(LyricsData {
                            name: omq
                                .as_ref()
                                .and_then(|o| o["Name"].as_str())
                                .unwrap_or(&track.title)
                                .to_string(),
                            author: track.author.clone(),
                            provider: "letrasmus".to_string(),
                            text: lines
                                .iter()
                                .map(|l| l.text.as_str())
                                .collect::<Vec<_>>()
                                .join("\n"),
                            lines: Some(lines),
                        });
                    }
                }
            }
        }

        // Fallback to plain lyrics from HTML
        let lyric_re =
            Regex::new(r#"(?i)<div class="lyric-original[^>]*">([\s\S]*?)</div>"#).unwrap();
        if let Some(c) = lyric_re.captures(&html) {
            let content = c
                .get(1)?
                .as_str()
                .replace("<br>", "\n")
                .replace("<p>", "")
                .replace("</p>", "\n");
            let tag_re = Regex::new(r#"<[^>]*>"#).unwrap();
            let cleaned = tag_re.replace_all(&content, "");
            let lines: Vec<LyricsLine> = cleaned
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .map(|l| LyricsLine {
                    text: l.to_string(),
                    timestamp: 0,
                    duration: 0,
                })
                .collect();

            if !lines.is_empty() {
                return Some(LyricsData {
                    name: omq
                        .as_ref()
                        .and_then(|o| o["Name"].as_str())
                        .unwrap_or(&track.title)
                        .to_string(),
                    author: track.author.clone(),
                    provider: "letrasmus".to_string(),
                    text: lines
                        .iter()
                        .map(|l| l.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    lines: None,
                });
            }
        }

        None
    }
}
