use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use regex::Regex;
use reqwest::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use serde_json::{Value, json};
use tracing::{debug, warn};

use super::token::PandoraTokenTracker;
use crate::{
    protocol::tracks::{LoadResult, PlaylistData, PlaylistInfo, SearchResult, Track, TrackInfo},
    sources::SourcePlugin,
};

const BASE_URL: &str = "https://www.pandora.com";
const ENDPOINT_SEARCH: &str = "/api/v3/sod/search";
const ENDPOINT_ANNOTATE: &str = "/api/v4/catalog/annotateObjects";
const ENDPOINT_DETAILS: &str = "/api/v4/catalog/getDetails";
const ENDPOINT_PLAYLIST_TRACKS: &str = "/api/v7/playlists/getTracks";
const ENDPOINT_ARTIST_ALL_TRACKS: &str = "/api/v4/catalog/getAllArtistTracksWithCollaborations";

fn url_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"https?://(?:www\.)?pandora\.com/(?:playlist/(?P<id>PL:[\d:]+)|artist/(?:[\w\-]+/)*(?P<id2>(?:TR|AL|AR)[A-Za-z0-9]+))").unwrap()
    })
}

pub struct PandoraSource {
    client: Arc<reqwest::Client>,
    token_tracker: Arc<PandoraTokenTracker>,
    search_limit: usize,
}

impl PandoraSource {
    pub fn new(
        config: Option<crate::config::PandoraConfig>,
        client: Arc<reqwest::Client>,
    ) -> Result<Self, String> {
        let (s_limit, csrf_override) = config
            .map(|c| (c.search_limit, c.csrf_token))
            .unwrap_or((10, None));

        let token_tracker = Arc::new(PandoraTokenTracker::new(client.clone(), csrf_override));
        token_tracker.clone().init();

        Ok(Self {
            client,
            token_tracker,
            search_limit: s_limit,
        })
    }

    async fn api_request(&self, path: &str, body: Value) -> Option<Value> {
        for is_retry in [false, true] {
            let tokens = self.token_tracker.get_tokens().await?;
            let url = format!("{BASE_URL}{path}");

            let resp = match self
                .base_request(self.client.post(&url))
                .header(ACCEPT, "application/json, text/plain, */*")
                .header(CONTENT_TYPE, "application/json")
                .header("origin", BASE_URL)
                .header("X-Csrftoken", &tokens.csrf_token_parsed)
                .header("X-Authtoken", &tokens.auth_token)
                .header("Cookie", &tokens.csrf_token_raw)
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    warn!("Pandora API request error for {path}: {e}");
                    return None;
                }
            };

            let status = resp.status();
            let body_res: Value = resp.json::<Value>().await.ok()?;

            let error_code = body_res
                .get("errorCode")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1);
            let error_string = body_res
                .get("errorString")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let is_error = !status.is_success()
                || (body_res.get("errorCode").is_some()
                    && !body_res.get("errorCode").unwrap().is_null());

            if is_error {
                if !is_retry
                    && (error_code == 1001 || error_string.contains("could not be validated"))
                {
                    debug!(
                        "Auth token error (code: {error_code}, message: {error_string}), refreshing..."
                    );
                    self.token_tracker.force_refresh().await;
                    continue;
                }

                warn!(
                    "Pandora API error for {path}: status {status}, code {error_code}, message {error_string}"
                );
                return None;
            }

            return Some(body_res);
        }
        None
    }

    fn get_artwork_url(&self, node: &Value) -> Option<String> {
        if let Some(icon) = node.get("icon").filter(|v| !v.is_null())
            && let Some(art_id) = icon
                .get("artId")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
        {
            return Some(format!(
                "https://content-images.p-cdn.com/{art_id}_1080W_1080H.jpg"
            ));
        }

        if let Some(thor_layers) = node
            .get("thorLayers")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            if let Some(grid) = thor_layers.strip_prefix("_;grid") {
                let encoded = urlencoding::encode(grid);
                return Some(format!(
                    "https://dyn-images.p-cdn.com/?l=_;grid{encoded}&w=1080&h=1080"
                ));
            }
            return Some(format!(
                "https://content-images.p-cdn.com/{thor_layers}_1080W_1080H.jpg"
            ));
        }

        None
    }

    fn map_track(&self, track: &Value, annotations: &Value) -> Option<Track> {
        let title = track.get("name").and_then(|v| v.as_str())?;
        let author = track
            .get("artistName")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Artist");
        let duration = track.get("duration").and_then(|v| v.as_i64()).unwrap_or(0) * 1000;
        if duration == 0 {
            return None;
        }

        let id = track.get("pandoraId").and_then(|v| v.as_str())?;
        let isrc = track
            .get("isrc")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());

        let album_id = track.get("albumId").and_then(|v| v.as_str()).unwrap_or("");
        let album = annotations.get(album_id).filter(|v| !v.is_null());
        let album_name = album.and_then(|v| v.get("name")).and_then(|v| v.as_str());
        let album_url = album
            .and_then(|v| v.get("shareableUrlPath"))
            .and_then(|v| v.as_str());

        let artist_id = track.get("artistId").and_then(|v| v.as_str()).unwrap_or("");
        let artist = annotations.get(artist_id).filter(|v| !v.is_null());
        let artist_url = artist
            .and_then(|v| v.get("shareableUrlPath"))
            .and_then(|v| v.as_str());
        let artist_artwork_url = artist.and_then(|v| self.get_artwork_url(v));

        let original_url = track
            .get("shareableUrlPath")
            .and_then(|v| v.as_str())
            .map(|p| format!("{BASE_URL}{p}"));
        let artwork_url = self.get_artwork_url(track);

        let info = TrackInfo {
            title: title.to_owned(),
            author: author.to_owned(),
            length: duration as u64,
            identifier: id.to_owned(),
            is_stream: false,
            uri: original_url,
            artwork_url,
            isrc,
            source_name: "pandora".to_owned(),
            is_seekable: true,
            position: 0,
        };

        let mut t = Track::new(info);
        t.plugin_info = json!({
            "albumName": album_name,
            "albumUrl": album_url.map(|p| format!("{BASE_URL}{p}")),
            "artistUrl": artist_url.map(|p| format!("{BASE_URL}{p}")),
            "artistArtworkUrl": artist_artwork_url,
            "previewUrl": null,
            "isPreview": false
        });
        Some(t)
    }

    fn build_annotate_request(&self, ids: &[String]) -> Value {
        json!({ "pandoraIds": ids })
    }

    fn find_by_url_suffix(&self, tail: &str, annotations: &Value) -> Value {
        if let Some(obj) = annotations.as_object() {
            for value in obj.values() {
                if let Some(path) = value.get("shareableUrlPath").and_then(|v| v.as_str())
                    && path.ends_with(&format!("/{}", tail))
                {
                    return value.clone();
                }
                if let Some(slug) = value.get("slugPlusPandoraId").and_then(|v| v.as_str())
                    && (slug.ends_with(tail) || slug.contains(tail))
                {
                    return value.clone();
                }
            }
        }
        Value::Null
    }

    async fn fetch_track(&self, id: &str) -> LoadResult {
        let data = match self
            .api_request(ENDPOINT_DETAILS, json!({ "pandoraId": id }))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };
        let annotations = data.get("annotations").unwrap_or(&Value::Null);
        let track = self.find_by_url_suffix(id, annotations);
        if track.is_null() {
            return LoadResult::Empty {};
        }

        self.map_track(&track, annotations)
            .map(LoadResult::Track)
            .unwrap_or(LoadResult::Empty {})
    }

    async fn get_album(&self, id: &str) -> LoadResult {
        let data = match self
            .api_request(ENDPOINT_DETAILS, json!({ "pandoraId": id }))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };
        let annotations = data.get("annotations").unwrap_or(&Value::Null);
        let album_node = self.find_by_url_suffix(id, annotations);
        if album_node.is_null() {
            return LoadResult::Empty {};
        }

        let name = album_node
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Album");
        let mut tracks = Vec::new();
        if let Some(track_ids) = album_node.get("tracks").and_then(|v| v.as_array()) {
            for tid in track_ids.iter().filter_map(|v| v.as_str()) {
                let t_node = annotations.get(tid).unwrap_or(&Value::Null);
                if !t_node.is_null()
                    && let Some(t) = self.map_track(t_node, annotations)
                {
                    tracks.push(t);
                }
            }
        }

        let url = album_node
            .get("shareableUrlPath")
            .and_then(|v| v.as_str())
            .map(|p| format!("{BASE_URL}{p}"));
        let artwork = self.get_artwork_url(&album_node);

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: name.to_owned(),
                selected_track: -1,
            },
            plugin_info: json!({
              "url": url,
              "type": "album",
              "artworkUrl": artwork,
              "totalTracks": tracks.len(),
              "author": album_node.get("artistName").and_then(|v| v.as_str())
            }),
            tracks,
        })
    }

    async fn get_artist(&self, id: &str) -> LoadResult {
        let data = match self
            .api_request(ENDPOINT_DETAILS, json!({ "pandoraId": id }))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };
        let annotations = data.get("annotations").unwrap_or(&Value::Null);
        let artist_node = self.find_by_url_suffix(id, annotations);
        if artist_node.is_null() {
            return LoadResult::Empty {};
        }

        let name = artist_node
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Artist");
        let top_tracks = data
            .get("artistDetails")
            .and_then(|v| v.get("topTracks"))
            .and_then(|v| v.as_array());

        let mut tracks = Vec::new();
        if let Some(ids) = top_tracks {
            for tid in ids.iter().filter_map(|v| v.as_str()) {
                let t_node = annotations.get(tid).unwrap_or(&Value::Null);
                if !t_node.is_null()
                    && let Some(t) = self.map_track(t_node, annotations)
                {
                    tracks.push(t);
                }
            }
        }

        let url = artist_node
            .get("shareableUrlPath")
            .and_then(|v| v.as_str())
            .map(|p| format!("{BASE_URL}{p}"));
        let artwork = self.get_artwork_url(&artist_node);

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: format!("{name}'s Top Tracks"),
                selected_track: -1,
            },
            plugin_info: json!({
              "url": url,
              "type": "artist",
              "artworkUrl": artwork,
              "totalTracks": tracks.len(),
              "author": name
            }),
            tracks,
        })
    }

    async fn get_playlist(&self, id: &str) -> LoadResult {
        let body = json!({
          "request": {
            "pandoraId": id,
            "playlistVersion": 0,
            "offset": 0,
            "limit": 5000,
            "annotationLimit": 100,
            "allowedTypes": ["TR"],
            "bypassPrivacyRules": true
          }
        });

        let json = match self.api_request(ENDPOINT_PLAYLIST_TRACKS, body).await {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let annotations = json.get("annotations").unwrap_or(&Value::Null);
        let tracks_node = json.get("tracks").and_then(|v| v.as_array());

        let mut merged = serde_json::Map::new();
        if let Some(obj) = annotations.as_object() {
            for (k, v) in obj {
                merged.insert(k.clone(), v.clone());
            }
        }

        let mut missing = Vec::new();
        if let Some(ts) = tracks_node {
            for t in ts {
                if let Some(pid) = t.get("pandoraId").and_then(|v| v.as_str())
                    && !merged.contains_key(pid)
                {
                    missing.push(pid.to_owned());
                }
            }
        }

        if !missing.is_empty()
            && let Some(extra) = self
                .api_request(ENDPOINT_ANNOTATE, self.build_annotate_request(&missing))
                .await
            && let Some(obj) = extra.as_object()
        {
            for (k, v) in obj {
                merged.insert(k.clone(), v.clone());
            }
        }

        let mut tracks = Vec::new();
        let merged_val = Value::Object(merged);
        if let Some(ts) = tracks_node {
            for t in ts {
                if let Some(pid) = t.get("pandoraId").and_then(|v| v.as_str())
                    && let Some(ann) = merged_val.get(pid)
                    && let Some(tr) = self.map_track(ann, &merged_val)
                {
                    tracks.push(tr);
                }
            }
        }

        let name = json
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Playlist");
        let url = json
            .get("shareableUrlPath")
            .and_then(|v| v.as_str())
            .map(|p| format!("{BASE_URL}{p}"));
        let artwork = self.get_artwork_url(&json);

        let mut author_name = None;
        if let Some(l_id) = json.get("listenerPandoraId").and_then(|v| v.as_str())
            && let Some(author) = annotations.get(l_id)
        {
            author_name = author.get("fullname").and_then(|v| v.as_str());
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: name.to_owned(),
                selected_track: -1,
            },
            plugin_info: json!({
              "url": url,
              "type": "playlist",
              "artworkUrl": artwork,
              "totalTracks": tracks.len(),
              "author": author_name
            }),
            tracks,
        })
    }

    async fn get_recommendations(&self, id: &str) -> LoadResult {
        let details = match self
            .api_request(ENDPOINT_DETAILS, json!({ "pandoraId": id }))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let similar = details
            .get("trackDetails")
            .and_then(|v| v.get("similarTracks"))
            .and_then(|v| v.as_array());

        let id_list: Vec<String> = similar
            .map(|s| {
                s.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_owned()))
                    .collect()
            })
            .unwrap_or_default();

        if id_list.is_empty() {
            return LoadResult::Empty {};
        }

        let annotations = match self
            .api_request(ENDPOINT_ANNOTATE, self.build_annotate_request(&id_list))
            .await
        {
            Some(a) => a,
            None => return LoadResult::Empty {},
        };

        let mut tracks = Vec::new();
        for tid in id_list {
            if let Some(item) = annotations.get(&tid).filter(|v| !v.is_null())
                && let Some(t) = self.map_track(item, &annotations)
            {
                tracks.push(t);
            }
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: "Pandora recommendations".to_owned(),
                selected_track: -1,
            },
            plugin_info: json!({
              "type": "recommendations",
              "totalTracks": tracks.len()
            }),
            tracks,
        })
    }

    async fn get_artist_all_songs(&self, id: &str) -> LoadResult {
        let body = json!({ "artistPandoraId": id, "annotationLimit": 100 });
        let json = match self.api_request(ENDPOINT_ARTIST_ALL_TRACKS, body).await {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let annotations = json.get("annotations").unwrap_or(&Value::Null);
        let tracks_node = json.get("tracks").and_then(|v| v.as_array());
        if tracks_node.as_ref().map(|n| n.is_empty()).unwrap_or(true) {
            return LoadResult::Empty {};
        }

        let mut merged = serde_json::Map::new();
        if let Some(obj) = annotations.as_object() {
            for (k, v) in obj {
                merged.insert(k.clone(), v.clone());
            }
        }

        let all_ids: Vec<String> = tracks_node
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_owned()))
            .collect();
        let mut missing = Vec::new();
        for tid in &all_ids {
            if !merged.contains_key(tid) {
                missing.push(tid.clone());
            }
        }

        if !missing.is_empty()
            && let Some(extra) = self
                .api_request(ENDPOINT_ANNOTATE, self.build_annotate_request(&missing))
                .await
            && let Some(obj) = extra.as_object()
        {
            for (k, v) in obj {
                merged.insert(k.clone(), v.clone());
            }
        }

        let merged_val = Value::Object(merged);
        let mut tracks = Vec::new();
        for tid in all_ids {
            if let Some(ann) = merged_val.get(&tid)
                && let Some(tr) = self.map_track(ann, &merged_val)
            {
                tracks.push(tr);
            }
        }

        let mut artist_node = self.find_by_url_suffix(id, annotations);
        if artist_node.is_null()
            && let Some(details) = self
                .api_request(ENDPOINT_DETAILS, json!({ "pandoraId": id }))
                .await
        {
            let details_ann = details.get("annotations").unwrap_or(&Value::Null);
            let match_node = self.find_by_url_suffix(id, details_ann);
            if !match_node.is_null() {
                artist_node = match_node;
            }
        }

        let name = artist_node
            .get("name")
            .and_then(|v| v.as_str())
            .map(|n| format!("{n} - All Songs"))
            .unwrap_or_else(|| "All Songs".to_owned());
        let url = artist_node
            .get("shareableUrlPath")
            .and_then(|v| v.as_str())
            .map(|p| format!("{BASE_URL}{p}"));
        let artwork = self.get_artwork_url(&artist_node);

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name,
                selected_track: -1,
            },
            plugin_info: json!({
              "url": url,
              "type": "artist",
              "artworkUrl": artwork,
              "totalTracks": tracks.len(),
              "author": artist_node.get("name").and_then(|v| v.as_str())
            }),
            tracks,
        })
    }

    async fn get_search(&self, query: &str) -> LoadResult {
        let body = json!({
          "query": query,
          "types": ["TR"],
          "listener": null,
          "start": 0,
          "count": 100,
          "annotate": true,
          "annotationRecipe": "CLASS_OF_2019"
        });

        let json = match self.api_request(ENDPOINT_SEARCH, body).await {
            Some(j) => j,
            None => return LoadResult::Empty {},
        };

        let annotations = json.get("annotations").unwrap_or(&Value::Null);
        let results = json.get("results").and_then(|v| v.as_array());
        if results.as_ref().map(|r| r.is_empty()).unwrap_or(true) {
            return LoadResult::Empty {};
        }

        let mut tracks = Vec::new();
        for v in results.unwrap() {
            if let Some(id) = v.as_str()
                && let Some(item) = annotations.get(id)
                && item.get("type").and_then(|v| v.as_str()) == Some("TR")
                && let Some(tr) = self.map_track(item, annotations)
            {
                tracks.push(tr);
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

    pub fn base_request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36")
    }

    async fn get_autocomplete(&self, query: &str, types: &[String]) -> Option<SearchResult> {
        let mut type_keys = Vec::new();
        if types.is_empty() {
            type_keys.extend_from_slice(&["TR", "AL", "AR", "PL"]);
        } else {
            for t in types {
                match t.as_str() {
                    "track" => type_keys.push("TR"),
                    "album" => type_keys.push("AL"),
                    "artist" => type_keys.push("AR"),
                    "playlist" => type_keys.push("PL"),
                    _ => {}
                }
            }
        }

        let body = json!({
          "query": query,
          "types": type_keys,
          "listener": null,
          "start": 0,
          "count": 100,
          "annotate": true,
          "annotationRecipe": "CLASS_OF_2019"
        });

        let json = self.api_request(ENDPOINT_SEARCH, body).await?;
        let annotations = json.get("annotations").unwrap_or(&Value::Null);
        let results = json.get("results").and_then(|v| v.as_array())?;

        let mut albums = Vec::new();
        let mut artists = Vec::new();
        let mut playlists = Vec::new();
        let mut tracks = Vec::new();

        for id_node in results {
            let id = id_node.as_str()?;
            let item = annotations.get(id).filter(|v| !v.is_null())?;
            let type_str = item.get("type").and_then(|v| v.as_str())?;

            match type_str {
                "TR" => {
                    if let Some(tr) = self.map_track(item, annotations) {
                        tracks.push(tr);
                    }
                }
                "AL" => {
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown Album");
                    let path = item.get("shareableUrlPath").and_then(|v| v.as_str());
                    let artwork = self.get_artwork_url(item);
                    let artist_name = item.get("artistName").and_then(|v| v.as_str());
                    albums.push(PlaylistData {
                        info: PlaylistInfo {
                            name: name.to_owned(),
                            selected_track: -1,
                        },
                        plugin_info: json!({
                          "url": path.map(|p| format!("{BASE_URL}{p}")),
                          "type": "album",
                          "artworkUrl": artwork,
                          "totalTracks": item.get("trackCount").and_then(|v| v.as_u64()).unwrap_or(0),
                          "author": artist_name
                        }),
                        tracks: Vec::new(),
                    });
                }
                "AR" => {
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Artist");
                    let path = item.get("shareableUrlPath").and_then(|v| v.as_str());
                    let artwork = self.get_artwork_url(item);
                    artists.push(PlaylistData {
                        info: PlaylistInfo {
                            name: format!("{name}'s Top Tracks"),
                            selected_track: -1,
                        },
                        plugin_info: json!({
                          "url": path.map(|p| format!("{BASE_URL}{p}")),
                          "type": "artist",
                          "artworkUrl": artwork,
                          "totalTracks": 0,
                          "author": name
                        }),
                        tracks: Vec::new(),
                    });
                }
                "PL" => {
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Playlist");
                    let path = item.get("shareableUrlPath").and_then(|v| v.as_str());
                    let artwork = self.get_artwork_url(item);
                    let mut author_name = None;
                    if let Some(l_id) = item.get("listenerPandoraId").and_then(|v| v.as_str())
                        && let Some(author) = annotations.get(l_id)
                    {
                        author_name = author.get("fullname").and_then(|v| v.as_str());
                    }
                    playlists.push(PlaylistData {
                        info: PlaylistInfo {
                            name: name.to_owned(),
                            selected_track: -1,
                        },
                        plugin_info: json!({
                          "url": path.map(|p| format!("{BASE_URL}{p}")),
                          "type": "playlist",
                          "artworkUrl": artwork,
                          "totalTracks": item.get("totalTracks").and_then(|v| v.as_u64()).unwrap_or(0),
                          "author": author_name
                        }),
                        tracks: Vec::new(),
                    });
                }
                _ => {}
            }
        }

        if tracks.len() > self.search_limit {
            tracks.truncate(self.search_limit);
        }

        Some(SearchResult {
            tracks,
            albums,
            artists,
            playlists,
            texts: Vec::new(),
            plugin: json!({}),
        })
    }
}

#[async_trait]
impl SourcePlugin for PandoraSource {
    fn name(&self) -> &str {
        "pandora"
    }

    fn can_handle(&self, identifier: &str) -> bool {
        self.search_prefixes()
            .iter()
            .any(|p| identifier.starts_with(p))
            || self
                .rec_prefixes()
                .iter()
                .any(|p| identifier.starts_with(p))
            || url_regex().is_match(identifier)
    }

    fn search_prefixes(&self) -> Vec<&str> {
        vec!["pdsearch:"]
    }

    fn is_mirror(&self) -> bool {
        true
    }

    fn rec_prefixes(&self) -> Vec<&str> {
        vec!["pdrec:"]
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
            if query.is_empty() {
                return LoadResult::Empty {};
            }
            return self.get_search(query).await;
        }

        if let Some(prefix) = self
            .rec_prefixes()
            .into_iter()
            .find(|p| identifier.starts_with(p))
        {
            let id = identifier.strip_prefix(prefix).unwrap();
            if id.is_empty() {
                return LoadResult::Empty {};
            }
            return self.get_recommendations(id).await;
        }

        let input = identifier.trim();
        if let Some(caps) = url_regex().captures(input)
            && let Some(id_match) = caps.name("id").or_else(|| caps.name("id2"))
        {
            let id = id_match.as_str();
            if id.is_empty() {
                return LoadResult::Empty {};
            }

            if let Some(tr_id) = id.strip_prefix("TR")
                && !tr_id.is_empty()
            {
                return self.fetch_track(id).await;
            }
            if let Some(al_id) = id.strip_prefix("AL")
                && !al_id.is_empty()
            {
                return self.get_album(id).await;
            }
            if let Some(ar_id) = id.strip_prefix("AR")
                && !ar_id.is_empty()
            {
                if input.contains("/artist/all-songs/") {
                    return self.get_artist_all_songs(id).await;
                }
                return self.get_artist(id).await;
            }
            if let Some(pl_id) = id.strip_prefix("PL:")
                && !pl_id.is_empty()
            {
                return self.get_playlist(id).await;
            }
        }

        LoadResult::Empty {}
    }

    async fn load_search(
        &self,
        query: &str,
        types: &[String],
        _routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<SearchResult> {
        self.get_autocomplete(query, types).await
    }

    async fn get_track(
        &self,
        _identifier: &str,
        _routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<crate::sources::plugin::BoxedTrack> {
        None
    }
}
