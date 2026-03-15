use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use regex::Regex;
use serde_json::{Value, json};
use tracing::error;

use crate::{
    protocol::tracks::{LoadResult, PlaylistData, PlaylistInfo, Track, TrackInfo},
    sources::{SourcePlugin, playable_track::BoxedTrack},
};

pub mod track;

static URL_PATTERN: OnceLock<Regex> = OnceLock::new();
static IDENTIFIER_PATTERN: OnceLock<Regex> = OnceLock::new();
static RESULT_BLOCKS_PATTERN: OnceLock<Regex> = OnceLock::new();
static ART_URL_PATTERN: OnceLock<Regex> = OnceLock::new();
static TITLE_PATTERN: OnceLock<Regex> = OnceLock::new();
static SUBHEAD_PATTERN: OnceLock<Regex> = OnceLock::new();
static ARTWORK_PATTERN: OnceLock<Regex> = OnceLock::new();
static TRALBUM_PATTERN: OnceLock<Regex> = OnceLock::new();

pub struct BandcampSource {
    client: Arc<reqwest::Client>,
    search_limit: usize,
}

impl BandcampSource {
    pub fn new(
        config: Option<crate::config::BandcampConfig>,
        client: Arc<reqwest::Client>,
    ) -> Result<Self, String> {
        Ok(Self {
            client,
            search_limit: config.map(|c| c.search_limit).unwrap_or(10),
        })
    }

    async fn search(&self, query: &str) -> LoadResult {
        let url = format!(
            "https://bandcamp.com/search?q={}&item_type=t&from=results",
            urlencoding::encode(query)
        );

        let resp = match self.base_request(self.client.get(url)).send().await {
            Ok(r) => r,
            Err(e) => {
                error!("Bandcamp search request failed: {e}");
                return LoadResult::Empty {};
            }
        };

        if !resp.status().is_success() {
            return LoadResult::Empty {};
        }

        let body = match resp.text().await {
            Ok(t) => t,
            Err(_) => return LoadResult::Empty {},
        };

        let result_blocks_re = RESULT_BLOCKS_PATTERN.get_or_init(|| {
            Regex::new(r"(?s)<li class=.searchresult data-search.[\s\S]*?</li>").unwrap()
        });
        let url_re = ART_URL_PATTERN
            .get_or_init(|| Regex::new(r#"<a class="artcont" href="([^"]+)">"#).unwrap());
        let title_re = TITLE_PATTERN.get_or_init(|| {
            Regex::new(r#"(?s)<div class="heading">\s*<a[^>]*>\s*(.+?)\s*</a>"#).unwrap()
        });
        let subhead_re = SUBHEAD_PATTERN
            .get_or_init(|| Regex::new(r#"(?s)<div class="subhead">([\s\S]*?)</div>"#).unwrap());
        let artwork_re = ARTWORK_PATTERN
            .get_or_init(|| Regex::new(r#"(?s)<div class="art">\s*<img src="([^"]+)""#).unwrap());

        let mut tracks = Vec::new();
        for block in result_blocks_re.find_iter(&body) {
            let block_str = block.as_str();

            if let (Some(url_m), Some(title_m), Some(subhead_m)) = (
                url_re.captures(block_str),
                title_re.captures(block_str),
                subhead_re.captures(block_str),
            ) {
                let uri = url_m[1].split('?').next().unwrap_or(&url_m[1]).to_owned();
                let title = title_m[1].trim().to_owned();
                let subhead = subhead_m[1].trim();
                let artist = subhead
                    .split(" de ")
                    .last()
                    .unwrap_or(subhead)
                    .trim()
                    .to_owned();
                let artwork_url = artwork_re.captures(block_str).map(|m| m[1].to_owned());

                tracks.push(Track::new(TrackInfo {
                    identifier: self.get_identifier_from_url(&uri),
                    is_seekable: true,
                    author: artist,
                    length: 0,
                    is_stream: false,
                    position: 0,
                    title,
                    uri: Some(uri),
                    artwork_url,
                    isrc: None,
                    source_name: "bandcamp".to_owned(),
                }));

                if tracks.len() >= self.search_limit {
                    break;
                }
            }
        }

        if tracks.is_empty() {
            LoadResult::Empty {}
        } else {
            LoadResult::Search(tracks)
        }
    }

    async fn resolve(&self, url: &str) -> LoadResult {
        let (tralbum_data, _) = match self.fetch_track_data(url).await {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let artist = tralbum_data["artist"]
            .as_str()
            .unwrap_or("Unknown Artist")
            .to_owned();
        let artwork_url = tralbum_data["art_id"]
            .as_u64()
            .map(|id| format!("https://f4.bcbits.com/img/a{id}_10.jpg"));

        if let Some(trackinfo) = tralbum_data["trackinfo"].as_array() {
            if trackinfo.len() > 1 {
                let mut tracks = Vec::new();
                for item in trackinfo {
                    let title = match item["title"].as_str() {
                        Some(t) => t.to_owned(),
                        None => continue,
                    };
                    if let Some(suffix) = item["title_link"].as_str() {
                        let track_url = if suffix.starts_with("http") {
                            suffix.to_owned()
                        } else {
                            let base = url.split(".bandcamp.com").next().unwrap_or("");
                            format!("{base}.bandcamp.com{suffix}")
                        };

                        let duration = (item["duration"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
                        let identifier = item["track_id"]
                            .as_u64()
                            .or(item["id"].as_u64())
                            .map(|id| id.to_string())
                            .unwrap_or_else(|| self.get_identifier_from_url(&track_url));

                        tracks.push(Track::new(TrackInfo {
                            identifier,
                            is_seekable: true,
                            author: artist.clone(),
                            length: duration,
                            is_stream: false,
                            position: 0,
                            title,
                            uri: Some(track_url),
                            artwork_url: artwork_url.clone(),
                            isrc: None,
                            source_name: "bandcamp".to_owned(),
                        }));
                    }
                }

                let playlist_name = tralbum_data["current"]["title"]
                    .as_str()
                    .unwrap_or("Bandcamp Album")
                    .to_owned();

                return LoadResult::Playlist(PlaylistData {
                    info: PlaylistInfo {
                        name: playlist_name,
                        selected_track: -1,
                    },
                    plugin_info: json!({}),
                    tracks,
                });
            } else if let Some(track_data) = trackinfo.first() {
                let title = track_data["title"]
                    .as_str()
                    .unwrap_or("Unknown Title")
                    .to_owned();
                let duration = (track_data["duration"].as_f64().unwrap_or(0.0) * 1000.0) as u64;
                let identifier = track_data["track_id"]
                    .as_u64()
                    .or(track_data["id"].as_u64())
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| self.get_identifier_from_url(url));

                return LoadResult::Track(Track::new(TrackInfo {
                    identifier,
                    is_seekable: true,
                    author: artist,
                    length: duration,
                    is_stream: false,
                    position: 0,
                    title,
                    uri: Some(url.to_owned()),
                    artwork_url,
                    isrc: None,
                    source_name: "bandcamp".to_owned(),
                }));
            }
        }

        LoadResult::Empty {}
    }

    async fn fetch_track_data(&self, url: &str) -> Option<(Value, Option<String>)> {
        let resp = self.base_request(self.client.get(url)).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }

        let body = resp.text().await.ok()?;

        let tralbum_re =
            TRALBUM_PATTERN.get_or_init(|| Regex::new(r#"data-tralbum=["'](.+?)["']"#).unwrap());
        let tralbum_data = if let Some(match_cap) = tralbum_re.captures(&body) {
            let decoded = match_cap[1].replace("&quot;", "\"");
            serde_json::from_str(&decoded).ok()?
        } else {
            return None;
        };

        let stream_url = track::extract_stream_url(&body);

        Some((tralbum_data, stream_url))
    }

    fn get_identifier_from_url(&self, url: &str) -> String {
        let url_re = URL_PATTERN.get_or_init(|| Regex::new(r"(?i)^https?://(?P<subdomain>[^/]+)\.bandcamp\.com/(?P<type>track|album)/(?P<slug>[^/?]+)").unwrap());
        if let Some(caps) = url_re.captures(url) {
            return format!("{}:{}", &caps["subdomain"], &caps["slug"]);
        }
        url.to_owned()
    }

    pub fn base_request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
    }
}

#[async_trait]
impl SourcePlugin for BandcampSource {
    fn name(&self) -> &str {
        "bandcamp"
    }

    fn can_handle(&self, identifier: &str) -> bool {
        let url_re = URL_PATTERN.get_or_init(|| Regex::new(r"(?i)^https?://(?P<subdomain>[^/]+)\.bandcamp\.com/(?P<type>track|album)/(?P<slug>[^/?]+)").unwrap());
        self.search_prefixes()
            .iter()
            .any(|p| identifier.starts_with(p))
            || url_re.is_match(identifier)
    }

    fn search_prefixes(&self) -> Vec<&str> {
        vec!["bcsearch:"]
    }

    async fn load(
        &self,
        identifier: &str,
        _routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> LoadResult {
        if let Some(prefix) = self
            .search_prefixes()
            .into_iter()
            .find(|p| identifier.starts_with(p))
        {
            return self.search(&identifier[prefix.len()..]).await;
        }

        let url_re = URL_PATTERN.get_or_init(|| Regex::new(r"(?i)^https?://(?P<subdomain>[^/]+)\.bandcamp\.com/(?P<type>track|album)/(?P<slug>[^/?]+)").unwrap());
        if url_re.is_match(identifier) {
            return self.resolve(identifier).await;
        }

        LoadResult::Empty {}
    }

    async fn get_track(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<BoxedTrack> {
        let id_re = IDENTIFIER_PATTERN.get_or_init(|| {
            Regex::new(r"^(?P<subdomain>[a-zA-Z0-9\-]+):(?P<slug>[a-zA-Z0-9\-]+)$").unwrap()
        });
        let url = if identifier.starts_with("http") {
            identifier.to_owned()
        } else if let Some(caps) = id_re.captures(identifier) {
            format!(
                "https://{}.bandcamp.com/track/{}",
                &caps["subdomain"], &caps["slug"]
            )
        } else {
            return None;
        };

        let (_, stream_url_opt) = self.fetch_track_data(&url).await?;
        let stream_url = stream_url_opt?;

        Some(Arc::new(track::BandcampTrack {
            client: self.client.clone(),
            uri: url,
            stream_url: Some(stream_url),
            local_addr: routeplanner.and_then(|rp| rp.get_address()),
        }))
    }
}
