use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

use async_trait::async_trait;
use rand::{Rng, distributions::Alphanumeric, thread_rng};
use regex::Regex;
use serde_json::Value;
use tracing::{error, warn};

use super::{track::AudiomackTrack, utils::build_auth_header};
use crate::{
    protocol::tracks::{LoadResult, PlaylistData, PlaylistInfo, Track, TrackInfo},
    sources::plugin::{PlayableTrack, SourcePlugin},
};

const API_BASE: &str = "https://api.audiomack.com/v1";

static SONG_REGEX: OnceLock<Regex> = OnceLock::new();
static ALBUM_REGEX: OnceLock<Regex> = OnceLock::new();
static PLAYLIST_REGEX: OnceLock<Regex> = OnceLock::new();
static ARTIST_REGEX: OnceLock<Regex> = OnceLock::new();
static LIKES_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct AudiomackSource {
    client: Arc<reqwest::Client>,
    search_limit: usize,
}

impl AudiomackSource {
    pub fn new(
        config: Option<crate::config::AudiomackConfig>,
        client: Arc<reqwest::Client>,
    ) -> Result<Self, String> {
        let search_limit = config.map(|c| c.search_limit).unwrap_or(20);

        Ok(Self {
            client,
            search_limit,
        })
    }

    fn generate_nonce(&self) -> String {
        thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    }

    async fn make_request(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        query_params: Option<BTreeMap<String, String>>,
    ) -> Option<Value> {
        let url = format!("{API_BASE}{endpoint}");
        tracing::debug!("Audiomack request: {method} {url} params: {query_params:?}");

        let mut request_builder = self.base_request(self.client.request(method.clone(), &url));

        let nonce = self.generate_nonce();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();

        let auth_header = build_auth_header(
            method.as_str(),
            &url,
            query_params.as_ref().unwrap_or(&BTreeMap::new()),
            &nonce,
            &timestamp,
        );
        request_builder = request_builder.header("Authorization", auth_header);

        if let Some(qp) = query_params {
            request_builder = request_builder.query(&qp);
        }

        let resp = match request_builder.send().await {
            Ok(r) => r,
            Err(e) => {
                error!("Audiomack request failed: {e}");
                return None;
            }
        };

        let status = resp.status();
        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                error!("Failed to read Audiomack response text: {e}");
                return None;
            }
        };

        if !status.is_success() {
            warn!("Audiomack API error status: {status} for endpoint: {endpoint}");
            return None;
        }

        match serde_json::from_str(&text) {
            Ok(v) => Some(v),
            Err(e) => {
                error!("Failed to parse Audiomack JSON: {e} body: {text}");
                None
            }
        }
    }

    fn base_request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36")
            .header("Accept", "application/json")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Origin", "https://audiomack.com")
            .header("Referer", "https://audiomack.com/")
            .header("Sec-Fetch-Site", "same-site")
            .header("Sec-Fetch-Mode", "cors")
            .header("Sec-Fetch-Dest", "empty")
            .header("Priority", "u=1, i")
            .header("DNT", "1")
            .header("sec-ch-ua-platform", "\"Windows\"")
    }

    fn parse_track(&self, json: &Value) -> Option<Track> {
        let id_val = json.get("id").or_else(|| json.get("song_id"));
        let id = match id_val {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => {
                tracing::debug!("Audiomack track missing id: {json:?}");
                return None;
            }
        };

        let title = json.get("title")?.as_str()?.to_owned();
        let author = json.get("artist")?.as_str()?.to_owned();

        let duration_sec = json
            .get("duration")
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_i64().map(|i| i as u64))
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .unwrap_or_default();

        let uploader_slug = json
            .pointer("/uploader/url_slug")
            .and_then(|v| v.as_str())
            .or_else(|| json.get("uploader_url_slug").and_then(|v| v.as_str()))
            .unwrap_or("unknown");

        let url_slug = json.get("url_slug")?.as_str()?;
        let uri = Some(format!(
            "https://audiomack.com/{uploader_slug}/song/{url_slug}"
        ));

        let artwork_url = json
            .get("image")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned());

        let isrc = json
            .get("isrc")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned());

        Some(Track::new(TrackInfo {
            identifier: id,
            is_seekable: true,
            author,
            length: duration_sec * 1000,
            is_stream: false,
            position: 0,
            title,
            uri,
            artwork_url,
            isrc,
            source_name: "audiomack".to_owned(),
        }))
    }

    async fn search(&self, query: &str) -> LoadResult {
        let mut params = BTreeMap::new();
        params.insert("q".to_owned(), query.to_owned());
        params.insert("limit".to_owned(), self.search_limit.to_string());
        params.insert("show".to_owned(), "songs".to_owned());
        params.insert("sort".to_owned(), "popular".to_owned());

        let json = match self
            .make_request(reqwest::Method::GET, "/search", Some(params))
            .await
        {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let results = match json.get("results").and_then(|v| v.as_array()) {
            Some(r) => r,
            None => return LoadResult::Empty {},
        };

        let tracks: Vec<_> = results
            .iter()
            .filter_map(|item| self.parse_track(item))
            .collect();

        if tracks.is_empty() {
            LoadResult::Empty {}
        } else {
            LoadResult::Search(tracks)
        }
    }

    async fn get_song(&self, artist: &str, slug: &str) -> LoadResult {
        let endpoint = format!("/music/song/{artist}/{slug}");
        let json = match self
            .make_request(reqwest::Method::GET, &endpoint, None)
            .await
        {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        if let Some(track) = json.get("results").and_then(|v| self.parse_track(v)) {
            LoadResult::Track(track)
        } else {
            LoadResult::Empty {}
        }
    }

    async fn get_playlist_items(&self, type_: &str, artist: &str, slug: &str) -> LoadResult {
        let endpoint = if type_ == "playlist" {
            format!("/playlist/{artist}/{slug}")
        } else {
            format!("/music/album/{artist}/{slug}")
        };

        let json = match self
            .make_request(reqwest::Method::GET, &endpoint, None)
            .await
        {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let results = match json.get("results") {
            Some(r) => r,
            None => return LoadResult::Empty {},
        };

        let name = results
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_owned();

        let tracks: Vec<_> = results
            .get("tracks")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| self.parse_track(item))
                    .collect()
            })
            .unwrap_or_default();

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        let url = results
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| format!("https://audiomack.com{s}"))
            .unwrap_or_else(|| format!("https://audiomack.com/{artist}/{type_}/{slug}"));

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name,
                selected_track: -1,
            },
            plugin_info: serde_json::json!({
                "type": type_,
                "url": url,
                "artworkUrl": results.get("image").and_then(|v| v.as_str()),
                "author": results.get("artist").and_then(|v| v.as_str()),
                "totalTracks": tracks.len()
            }),
            tracks,
        })
    }

    async fn get_artist(&self, artist_slug: &str) -> LoadResult {
        let json = match self
            .make_request(
                reqwest::Method::GET,
                &format!("/artist/{artist_slug}"),
                None,
            )
            .await
        {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let results = match json.get("results") {
            Some(r) => r,
            None => return LoadResult::Empty {},
        };

        let artist_id = results
            .get("id")
            .and_then(|v| {
                v.as_str()
                    .map(|s| s.to_owned())
                    .or_else(|| v.as_i64().map(|i| i.to_string()))
            })
            .unwrap_or_default();

        if artist_id.is_empty() {
            return LoadResult::Empty {};
        }

        let name = results
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Artist")
            .to_owned();

        let mut params = BTreeMap::new();
        params.insert("artist_id".to_owned(), artist_id);
        params.insert("limit".to_owned(), "100".to_owned());
        params.insert("sort".to_owned(), "rank".to_owned());
        params.insert("type".to_owned(), "songs".to_owned());

        let tracks_json = match self
            .make_request(reqwest::Method::GET, "/search_artist_content", Some(params))
            .await
        {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let tracks: Vec<_> = tracks_json
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| self.parse_track(item))
                    .collect()
            })
            .unwrap_or_default();

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        let url = results
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| format!("https://audiomack.com{s}"))
            .unwrap_or_else(|| format!("https://audiomack.com/{artist_slug}"));

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: format!("{name}'s Top Tracks"),
                selected_track: -1,
            },
            plugin_info: serde_json::json!({
                "type": "artist",
                "url": url,
                "artworkUrl": results.get("image").and_then(|v| v.as_str()),
                "author": name,
                "totalTracks": tracks.len()
            }),
            tracks,
        })
    }

    async fn get_artist_likes(&self, artist_slug: &str) -> LoadResult {
        let json = match self
            .make_request(
                reqwest::Method::GET,
                &format!("/artist/{artist_slug}"),
                None,
            )
            .await
        {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let results = match json.get("results") {
            Some(r) => r,
            None => return LoadResult::Empty {},
        };

        let name = results
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Artist")
            .to_owned();

        let likes_json = match self
            .make_request(
                reqwest::Method::GET,
                &format!("/artist/{artist_slug}/favorites"),
                None,
            )
            .await
        {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let tracks: Vec<_> = likes_json
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| self.parse_track(item))
                    .collect()
            })
            .unwrap_or_default();

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: format!("{name}'s Liked Tracks"),
                selected_track: -1,
            },
            plugin_info: serde_json::json!({}),
            tracks,
        })
    }
}

#[async_trait]
impl SourcePlugin for AudiomackSource {
    fn name(&self) -> &str {
        "audiomack"
    }

    fn can_handle(&self, identifier: &str) -> bool {
        self.search_prefixes()
            .iter()
            .any(|p| identifier.starts_with(p))
            || SONG_REGEX
                .get_or_init(|| Regex::new(r"https?://(?:www\.)?audiomack\.com/(?P<artist>[^/]+)/song/(?P<slug>[^/?#]+)").unwrap())
                .is_match(identifier)
            || ALBUM_REGEX
                .get_or_init(|| Regex::new(r"https?://(?:www\.)?audiomack\.com/(?P<artist>[^/]+)/album/(?P<slug>[^/?#]+)").unwrap())
                .is_match(identifier)
            || PLAYLIST_REGEX
                .get_or_init(|| Regex::new(r"https?://(?:www\.)?audiomack\.com/(?P<artist>[^/]+)/playlist/(?P<slug>[^/?#]+)").unwrap())
                .is_match(identifier)
            || ARTIST_REGEX
                .get_or_init(|| Regex::new(r"https?://(?:www\.)?audiomack\.com/(?P<artist>[^/?#]+)(?:/songs)?/?$").unwrap())
                .is_match(identifier)
            || LIKES_REGEX
                .get_or_init(|| Regex::new(r"https?://(?:www\.)?audiomack\.com/(?P<artist>[^/]+)/likes").unwrap())
                .is_match(identifier)
    }

    fn search_prefixes(&self) -> Vec<&str> {
        vec!["amksearch:"]
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
            let query = identifier.strip_prefix(prefix).unwrap();
            return self.search(query).await;
        }

        if let Some(caps) = SONG_REGEX.get().and_then(|r| r.captures(identifier)) {
            let artist = caps.name("artist").map(|m| m.as_str()).unwrap_or("");
            let slug = caps.name("slug").map(|m| m.as_str()).unwrap_or("");
            return self.get_song(artist, slug).await;
        }

        if let Some(caps) = ALBUM_REGEX.get().and_then(|r| r.captures(identifier)) {
            let artist = caps.name("artist").map(|m| m.as_str()).unwrap_or("");
            let slug = caps.name("slug").map(|m| m.as_str()).unwrap_or("");
            return self.get_playlist_items("album", artist, slug).await;
        }

        if let Some(caps) = PLAYLIST_REGEX.get().and_then(|r| r.captures(identifier)) {
            let artist = caps.name("artist").map(|m| m.as_str()).unwrap_or("");
            let slug = caps.name("slug").map(|m| m.as_str()).unwrap_or("");
            return self.get_playlist_items("playlist", artist, slug).await;
        }

        if let Some(caps) = LIKES_REGEX.get().and_then(|r| r.captures(identifier)) {
            let artist = caps.name("artist").map(|m| m.as_str()).unwrap_or("");
            return self.get_artist_likes(artist).await;
        }

        if let Some(caps) = ARTIST_REGEX.get().and_then(|r| r.captures(identifier)) {
            let artist = caps.name("artist").map(|m| m.as_str()).unwrap_or("");
            return self.get_artist(artist).await;
        }

        LoadResult::Empty {}
    }

    async fn get_track(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<Box<dyn PlayableTrack>> {
        let mut track_id = identifier.to_owned();

        if SONG_REGEX
            .get()
            .map(|r| r.is_match(identifier))
            .unwrap_or(false)
        {
            if let LoadResult::Track(track) = self.load(identifier, None).await {
                track_id = track.info.identifier;
            } else {
                return None;
            }
        }

        Some(Box::new(AudiomackTrack {
            client: self.client.clone(),
            identifier: track_id,
            local_addr: routeplanner.and_then(|rp| rp.get_address()),
        }))
    }

    fn get_proxy_config(&self) -> Option<crate::config::HttpProxyConfig> {
        None
    }
}
