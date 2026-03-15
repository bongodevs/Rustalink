use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use regex::Regex;
use serde_json::{Value, json};

use crate::{
    protocol::tracks::{LoadResult, PlaylistData, PlaylistInfo, Track, TrackInfo},
    sources::{SourcePlugin, playable_track::BoxedTrack},
};

pub mod track;

const API_BASE: &str = "https://discoveryprovider.audius.co";

static TRACK_PATTERN: OnceLock<Regex> = OnceLock::new();
static PLAYLIST_PATTERN: OnceLock<Regex> = OnceLock::new();
static ALBUM_PATTERN: OnceLock<Regex> = OnceLock::new();
static USER_PATTERN: OnceLock<Regex> = OnceLock::new();

pub struct AudiusSource {
    client: Arc<reqwest::Client>,
    app_name: String,
    search_limit: usize,
    playlist_load_limit: usize,
    album_load_limit: usize,
}

impl AudiusSource {
    pub fn new(
        config: Option<crate::config::AudiusConfig>,
        client: Arc<reqwest::Client>,
    ) -> Result<Self, String> {
        let config = config.unwrap_or_default();

        Ok(Self {
            client,
            app_name: config.app_name.unwrap_or_else(|| "Rustalink".to_owned()),
            search_limit: config.search_limit,
            playlist_load_limit: config.playlist_load_limit,
            album_load_limit: config.album_load_limit,
        })
    }

    async fn api_request(
        &self,
        endpoint: &str,
        query: Option<std::collections::BTreeMap<String, String>>,
    ) -> Option<Value> {
        let url = format!("{API_BASE}{endpoint}");
        let mut builder = self.client.get(&url);

        if let Some(q) = query {
            builder = builder.query(&q);
        }
        builder = builder.query(&[("app_name", &self.app_name)]);

        let resp = builder.send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }

        let body: Value = resp.json().await.ok()?;
        Some(body["data"].clone())
    }

    async fn search(&self, query: &str) -> LoadResult {
        let mut params = std::collections::BTreeMap::new();
        params.insert("query".to_owned(), query.to_owned());
        params.insert("limit".to_owned(), self.search_limit.to_string());

        let data = match self.api_request("/v1/tracks/search", Some(params)).await {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let tracks = self.parse_tracks(&data);
        if tracks.is_empty() {
            LoadResult::Empty {}
        } else {
            LoadResult::Search(tracks)
        }
    }

    async fn resolve_url(&self, url: &str) -> LoadResult {
        if PLAYLIST_PATTERN
            .get_or_init(|| Regex::new(r"(?i)^https?://(?:www\.)?audius\.co/(?P<artist>[^/]+)/playlist/(?P<slug>[^/?#]+)(?:\?.*)?$").unwrap())
            .is_match(url)
        {
            return self.resolve_playlist_or_album(url, "playlist").await;
        }
        if ALBUM_PATTERN
            .get_or_init(|| Regex::new(r"(?i)^https?://(?:www\.)?audius\.co/(?P<artist>[^/]+)/album/(?P<slug>[^/?#]+)(?:\?.*)?$").unwrap())
            .is_match(url)
        {
            return self.resolve_playlist_or_album(url, "album").await;
        }
        if TRACK_PATTERN
            .get_or_init(|| Regex::new(r"(?i)^https?://(?:www\.)?audius\.co/(?P<artist>[^/]+)/(?P<slug>[^/?#]+)(?:\?.*)?$").unwrap())
            .is_match(url)
        {
            return self.resolve_track(url).await;
        }
        if USER_PATTERN
            .get_or_init(|| {
                Regex::new(r"(?i)^https?://(?:www\.)?audius\.co/(?P<user>[^/?#]+)(?:\?.*)?$")
                    .unwrap()
            })
            .is_match(url)
        {
            return self.resolve_user(url).await;
        }

        LoadResult::Empty {}
    }

    async fn resolve_track(&self, url: &str) -> LoadResult {
        let mut params = std::collections::BTreeMap::new();
        params.insert("url".to_owned(), url.to_owned());

        let data = match self.api_request("/v1/resolve", Some(params)).await {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        match self.build_track(&data) {
            Some(t) => LoadResult::Track(t),
            None => LoadResult::Empty {},
        }
    }

    async fn resolve_playlist_or_album(&self, url: &str, type_: &str) -> LoadResult {
        let mut params = std::collections::BTreeMap::new();
        params.insert("url".to_owned(), url.to_owned());

        let data = match self.api_request("/v1/resolve", Some(params)).await {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let id = match data["id"].as_str() {
            Some(i) => i,
            None => return LoadResult::Empty {},
        };

        let limit = if type_ == "playlist" {
            self.playlist_load_limit
        } else {
            self.album_load_limit
        };

        let mut tracks_params = std::collections::BTreeMap::new();
        tracks_params.insert("limit".to_owned(), limit.to_string());

        let tracks_data = match self
            .api_request(&format!("/v1/playlists/{id}/tracks"), Some(tracks_params))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let tracks = self.parse_tracks(&tracks_data);
        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        let name = data["playlist_name"]
            .as_str()
            .unwrap_or("Audius Playlist")
            .to_owned();

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name,
                selected_track: -1,
            },
            plugin_info: json!({}),
            tracks,
        })
    }

    async fn resolve_user(&self, url: &str) -> LoadResult {
        let mut params = std::collections::BTreeMap::new();
        params.insert("url".to_owned(), url.to_owned());

        let data = match self.api_request("/v1/resolve", Some(params)).await {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let id = match data["id"].as_str() {
            Some(i) => i,
            None => return LoadResult::Empty {},
        };

        let mut tracks_params = std::collections::BTreeMap::new();
        tracks_params.insert("limit".to_owned(), self.search_limit.to_string());

        let tracks_data = match self
            .api_request(&format!("/v1/users/{id}/tracks"), Some(tracks_params))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let tracks = self.parse_tracks(&tracks_data);
        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        let name = format!("{}'s Tracks", data["name"].as_str().unwrap_or("Artist"));

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name,
                selected_track: -1,
            },
            plugin_info: json!({}),
            tracks,
        })
    }

    fn parse_tracks(&self, data: &Value) -> Vec<Track> {
        data.as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| self.build_track(item))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn build_track(&self, data: &Value) -> Option<Track> {
        let id = data["id"].as_str()?;
        let title = data["title"].as_str()?.to_owned();
        let author = data["user"]["name"]
            .as_str()
            .unwrap_or("Unknown Artist")
            .to_owned();
        let duration = (data["duration"].as_f64().unwrap_or(0.0) * 1000.0) as u64;

        let uri = data["permalink"].as_str().map(|p| {
            if p.starts_with("http") {
                p.to_owned()
            } else {
                format!("https://audius.co{p}")
            }
        });

        let artwork_url = self.get_artwork_url(&data["artwork"]);

        Some(Track::new(TrackInfo {
            identifier: id.to_owned(),
            is_seekable: true,
            author,
            length: duration,
            is_stream: false,
            position: 0,
            title,
            uri,
            artwork_url,
            isrc: None,
            source_name: "audius".to_owned(),
        }))
    }

    fn get_artwork_url(&self, artwork: &Value) -> Option<String> {
        if artwork.is_null() {
            return None;
        }

        if let Some(url) = artwork.as_str() {
            return Some(if url.starts_with('/') {
                format!("https://audius.co{url}")
            } else {
                url.to_owned()
            });
        }

        for size in &["480x480", "1000x1000", "150x150"] {
            if let Some(url) = artwork[size].as_str() {
                return Some(if url.starts_with('/') {
                    format!("https://audius.co{url}")
                } else {
                    url.to_owned()
                });
            }
        }
        None
    }
}

#[async_trait]
impl SourcePlugin for AudiusSource {
    fn name(&self) -> &str {
        "audius"
    }

    fn can_handle(&self, identifier: &str) -> bool {
        self.search_prefixes()
            .iter()
            .any(|p| identifier.starts_with(p))
            || TRACK_PATTERN
                .get_or_init(|| Regex::new(r"(?i)^https?://(?:www\.)?audius\.co/(?P<artist>[^/]+)/(?P<slug>[^/?#]+)(?:\?.*)?$").unwrap())
                .is_match(identifier)
            || PLAYLIST_PATTERN
                .get_or_init(|| Regex::new(r"(?i)^https?://(?:www\.)?audius\.co/(?P<artist>[^/]+)/playlist/(?P<slug>[^/?#]+)(?:\?.*)?$").unwrap())
                .is_match(identifier)
            || ALBUM_PATTERN
                .get_or_init(|| Regex::new(r"(?i)^https?://(?:www\.)?audius\.co/(?P<artist>[^/]+)/album/(?P<slug>[^/?#]+)(?:\?.*)?$").unwrap())
                .is_match(identifier)
            || USER_PATTERN
                .get_or_init(|| Regex::new(r"(?i)^https?://(?:www\.)?audius\.co/(?P<user>[^/?#]+)(?:\?.*)?$").unwrap())
                .is_match(identifier)
    }

    fn search_prefixes(&self) -> Vec<&str> {
        vec!["ausearch:", "audsearch:"]
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

        self.resolve_url(identifier).await
    }

    async fn get_track(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<BoxedTrack> {
        let track_id = if identifier.starts_with("http") {
            let mut params = std::collections::BTreeMap::new();
            params.insert("url".to_owned(), identifier.to_owned());

            let data = self.api_request("/v1/resolve", Some(params)).await?;
            data["id"].as_str()?.to_owned()
        } else {
            identifier.to_owned()
        };

        let stream_url = track::fetch_stream_url(&self.client, &track_id, &self.app_name).await?;

        Some(Arc::new(track::AudiusTrack {
            client: self.client.clone(),
            track_id,
            stream_url: Some(stream_url),
            app_name: self.app_name.clone(),
            local_addr: routeplanner.and_then(|rp| rp.get_address()),
        }))
    }
}
