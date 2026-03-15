use std::sync::Arc;

use async_trait::async_trait;
use regex::Regex;
use serde_json::{Value, json};

use super::{
    api::VkApiClient,
    track::VkMusicTrack,
    utils::{extract_thumbnail, unmask_vk_url},
};
use crate::{
    config::VkMusicConfig,
    protocol::tracks::{LoadResult, PlaylistData, PlaylistInfo, Track, TrackInfo},
    sources::{SourcePlugin, playable_track::BoxedTrack},
};

pub struct VkMusicSource {
    api: VkApiClient,
    proxy: Option<crate::config::HttpProxyConfig>,
    search_limit: usize,
    playlist_track_limit: usize,
    artist_track_limit: usize,
    rec_limit: usize,
    track_re: Regex,
    playlist_z_re: Regex,
    playlist_path_re: Regex,
    artist_re: Regex,
    audios_re: Regex,
}

impl VkMusicSource {
    pub fn new(
        config: Option<VkMusicConfig>,
        client: Arc<reqwest::Client>,
    ) -> Result<Self, String> {
        let cfg = config.ok_or("VK Music configuration is missing")?;

        let api = VkApiClient::new(client, cfg.user_token.clone(), cfg.user_cookie.clone());

        Ok(Self {
            api,
            proxy: cfg.proxy,
            search_limit: cfg.search_limit,
            playlist_track_limit: cfg.playlist_load_limit * 50,
            artist_track_limit: cfg.artist_load_limit * 10,
            rec_limit: cfg.recommendations_load_limit,
            track_re: Regex::new(
                r"(?i)https?://vk\.(?:com|ru)/audio(?P<owner>-?\d+)_(?P<id>\d+)(?:_(?P<hash>[a-z0-9]+))?",
            ).unwrap(),
            playlist_z_re: Regex::new(
                r"(?i)https?://vk\.(?:com|ru)/audios-?\d+\?[^#]*z=audio_playlist(?P<owner>-?\d+)_(?P<id>\d+)(?:(?:%2F|_)(?P<hash>[a-z0-9]+))?",
            ).unwrap(),
            playlist_path_re: Regex::new(
                r"(?i)https?://vk\.(?:com|ru)/music/(?:playlist|album)/(?P<owner>-?\d+)_(?P<id>\d+)(?:_(?P<hash>[a-z0-9]+))?",
            ).unwrap(),
            artist_re: Regex::new(
                r"(?i)https?://vk\.(?:com|ru)/artist/(?P<slug>[^/?#\s]+)"
            ).unwrap(),
            audios_re: Regex::new(
                r"(?i)^https?://vk\.(?:com|ru)/audios(?P<owner>-?\d+)\s*$"
            ).unwrap(),
        })
    }

    async fn search(&self, q: &str) -> LoadResult {
        let resp = self
            .api
            .call(
                "audio.search",
                &[
                    ("q", q.to_string()),
                    ("count", self.search_limit.to_string()),
                    ("sort", "2".to_string()),
                ],
            )
            .await;

        match resp.as_ref().and_then(|r| r["items"].as_array()) {
            Some(items) if !items.is_empty() => {
                let tracks: Vec<Track> = items.iter().filter_map(|i| self.build_track(i)).collect();
                if tracks.is_empty() {
                    LoadResult::Empty {}
                } else {
                    LoadResult::Search(tracks)
                }
            }
            _ => LoadResult::Empty {},
        }
    }

    async fn recommendations(&self, target: &str) -> LoadResult {
        let resp = self
            .api
            .call(
                "audio.getRecommendations",
                &[
                    ("target_audio", target.to_string()),
                    ("count", self.rec_limit.to_string()),
                ],
            )
            .await;

        let items = match resp.as_ref().and_then(|r| r["items"].as_array()) {
            Some(arr) if !arr.is_empty() => arr.clone(),
            _ => return LoadResult::Empty {},
        };

        let tracks: Vec<Track> = items.iter().filter_map(|i| self.build_track(i)).collect();
        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: "VK Music Recommendations".to_string(),
                selected_track: -1,
            },
            plugin_info: json!({ "type": "recommendations" }),
            tracks,
        })
    }

    async fn load_playlist(&self, owner: &str, id: &str, access_key: Option<&str>) -> LoadResult {
        let mut params = vec![
            ("owner_id", owner.to_string()),
            ("album_id", id.to_string()),
            ("count", self.playlist_track_limit.to_string()),
        ];
        if let Some(key) = access_key {
            params.push(("access_key", key.to_string()));
        }

        let resp = self.api.call("audio.get", &params).await;
        let items = match resp.as_ref().and_then(|r| r["items"].as_array()) {
            Some(arr) if !arr.is_empty() => arr.clone(),
            _ => return LoadResult::Empty {},
        };

        let tracks: Vec<Track> = items.iter().filter_map(|i| self.build_track(i)).collect();
        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        let mut meta_params = vec![
            ("owner_id", owner.to_string()),
            ("playlist_id", id.to_string()),
        ];
        if let Some(key) = access_key {
            meta_params.push(("access_key", key.to_string()));
        }

        let meta = self.api.call("audio.getPlaylistById", &meta_params).await;
        let title = meta
            .as_ref()
            .and_then(|m| m["title"].as_str())
            .unwrap_or("VK Music Playlist")
            .to_string();
        let pl_type = if meta.as_ref().and_then(|m| m["type"].as_i64()) == Some(1) {
            "album"
        } else {
            "playlist"
        };

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: title,
                selected_track: -1,
            },
            plugin_info: json!({ "type": pl_type }),
            tracks,
        })
    }

    async fn load_track(&self, audio_id: &str) -> LoadResult {
        let resp = self
            .api
            .call("audio.getById", &[("audios", audio_id.to_string())])
            .await;
        match resp
            .as_ref()
            .and_then(|r| r.as_array())
            .and_then(|a| a.first())
        {
            Some(item) => match self.build_track(item) {
                Some(t) => LoadResult::Track(t),
                None => LoadResult::Empty {},
            },
            None => LoadResult::Empty {},
        }
    }

    async fn load_artist(&self, slug: &str) -> LoadResult {
        let artist = self
            .api
            .call("audio.getArtistById", &[("artist_id", slug.to_string())])
            .await;
        let artist_id = artist.as_ref().and_then(|r| {
            r["id"]
                .as_str()
                .map(String::from)
                .or_else(|| r["id"].as_i64().map(|n| n.to_string()))
        });
        let name = artist
            .as_ref()
            .and_then(|r| r["name"].as_str())
            .unwrap_or(slug)
            .to_string();
        let resolved_id = artist_id.as_deref().unwrap_or(slug);

        let resp = self
            .api
            .call(
                "audio.getAudiosByArtist",
                &[
                    ("artist_id", resolved_id.to_string()),
                    ("count", self.artist_track_limit.to_string()),
                ],
            )
            .await;

        let items = match resp.as_ref().and_then(|r| r["items"].as_array()) {
            Some(arr) if !arr.is_empty() => arr.clone(),
            _ => return LoadResult::Empty {},
        };

        let tracks: Vec<Track> = items.iter().filter_map(|i| self.build_track(i)).collect();
        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: format!("{}'s Top Tracks", name),
                selected_track: -1,
            },
            plugin_info: json!({ "type": "artist" }),
            tracks,
        })
    }

    async fn load_user_wall(&self, owner_id: &str) -> LoadResult {
        let resp = self
            .api
            .call(
                "audio.get",
                &[
                    ("owner_id", owner_id.to_string()),
                    ("count", self.playlist_track_limit.to_string()),
                ],
            )
            .await;

        let items = match resp.as_ref().and_then(|r| r["items"].as_array()) {
            Some(arr) if !arr.is_empty() => arr.clone(),
            _ => return LoadResult::Empty {},
        };

        let tracks: Vec<Track> = items.iter().filter_map(|i| self.build_track(i)).collect();
        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: "VK Music".to_string(),
                selected_track: -1,
            },
            plugin_info: json!({ "type": "playlist" }),
            tracks,
        })
    }

    pub async fn resolve_stream_url(
        &self,
        audio_id: &str,
        access_key: Option<&str>,
    ) -> Option<String> {
        let audios_param = match access_key {
            Some(k) => format!("{}_{}", audio_id, k),
            None => audio_id.to_string(),
        };

        let resp = self
            .api
            .call("audio.getById", &[("audios", audios_param)])
            .await;
        let raw_url = resp
            .as_ref()
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item["url"].as_str())
            .filter(|u| !u.is_empty())
            .map(String::from)?;

        let uid = *self.api.user_id.read().await;
        Some(unmask_vk_url(&raw_url, uid))
    }

    fn build_track(&self, item: &Value) -> Option<Track> {
        let owner_id = item["owner_id"].as_i64()?;
        let track_id = item["id"].as_i64()?;
        let audio_id = format!("{}_{}", owner_id, track_id);

        Some(Track::new(TrackInfo {
            identifier: audio_id.clone(),
            is_seekable: true,
            author: item["artist"]
                .as_str()
                .unwrap_or("Unknown Artist")
                .to_string(),
            length: item["duration"].as_u64().unwrap_or(0) * 1000,
            is_stream: false,
            position: 0,
            title: item["title"].as_str().unwrap_or("Unknown").to_string(),
            uri: Some(format!("https://vk.com/audio{}", audio_id)),
            artwork_url: extract_thumbnail(item),
            isrc: None,
            source_name: "vkmusic".to_string(),
        }))
    }

    fn build_playlist_entry(&self, item: &Value, kind: &str) -> Option<PlaylistData> {
        let title = item["title"].as_str()?;
        let owner_id = item["owner_id"].as_i64()?;
        let id = item["id"].as_i64()?;
        let access_key = item["access_key"].as_str();

        let uri = match access_key {
            Some(key) => format!("https://vk.com/music/{}/{}_{}_{}", kind, owner_id, id, key),
            None => format!("https://vk.com/music/{}/{}_{}", kind, owner_id, id),
        };

        Some(PlaylistData {
            info: PlaylistInfo {
                name: title.to_string(),
                selected_track: -1,
            },
            plugin_info: json!({ "type": kind, "uri": uri }),
            tracks: Vec::new(),
        })
    }

    fn build_artist_entry(&self, item: &Value) -> Option<PlaylistData> {
        let name = item["name"].as_str()?;
        let domain = item["domain"].as_str().unwrap_or(name);
        let artwork = item["photo"]
            .as_array()
            .and_then(|photos| {
                photos
                    .iter()
                    .max_by_key(|p| p["width"].as_u64().unwrap_or(0))
            })
            .and_then(|p| p["url"].as_str())
            .map(String::from);

        Some(PlaylistData {
            info: PlaylistInfo {
                name: format!("{}'s Top Tracks", name),
                selected_track: -1,
            },
            plugin_info: json!({ "type": "artist", "artworkUrl": artwork, "uri": format!("https://vk.com/artist/{}", domain) }),
            tracks: Vec::new(),
        })
    }
}

#[async_trait]
impl SourcePlugin for VkMusicSource {
    fn name(&self) -> &str {
        "vkmusic"
    }

    fn can_handle(&self, id: &str) -> bool {
        self.search_prefixes().iter().any(|p| id.starts_with(p))
            || self.rec_prefixes().iter().any(|p| id.starts_with(p))
            || self.track_re.is_match(id)
            || self.playlist_z_re.is_match(id)
            || self.playlist_path_re.is_match(id)
            || self.artist_re.is_match(id)
            || self.audios_re.is_match(id)
    }

    fn search_prefixes(&self) -> Vec<&str> {
        vec!["vksearch:"]
    }

    fn rec_prefixes(&self) -> Vec<&str> {
        vec!["vkrec:"]
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
            return self.search(identifier.strip_prefix(prefix).unwrap()).await;
        }

        if let Some(prefix) = self
            .rec_prefixes()
            .into_iter()
            .find(|p| identifier.starts_with(p))
        {
            return self
                .recommendations(identifier.strip_prefix(prefix).unwrap())
                .await;
        }

        if let Some(caps) = self.playlist_z_re.captures(identifier) {
            let owner = caps.name("owner").map(|m| m.as_str()).unwrap_or("");
            let id = caps.name("id").map(|m| m.as_str()).unwrap_or("");
            return self
                .load_playlist(owner, id, caps.name("hash").map(|m| m.as_str()))
                .await;
        }

        if let Some(caps) = self.playlist_path_re.captures(identifier) {
            let owner = caps.name("owner").map(|m| m.as_str()).unwrap_or("");
            let id = caps.name("id").map(|m| m.as_str()).unwrap_or("");
            return self
                .load_playlist(owner, id, caps.name("hash").map(|m| m.as_str()))
                .await;
        }

        if let Some(caps) = self.track_re.captures(identifier) {
            let owner = caps.name("owner").map(|m| m.as_str()).unwrap_or("");
            let id = caps.name("id").map(|m| m.as_str()).unwrap_or("");
            return self.load_track(&format!("{}_{}", owner, id)).await;
        }

        if let Some(caps) = self.artist_re.captures(identifier) {
            return self
                .load_artist(caps.name("slug").map(|m| m.as_str()).unwrap_or(""))
                .await;
        }

        if let Some(caps) = self.audios_re.captures(identifier) {
            return self
                .load_user_wall(caps.name("owner").map(|m| m.as_str()).unwrap_or(""))
                .await;
        }

        LoadResult::Empty {}
    }

    async fn load_search(
        &self,
        query: &str,
        _types: &[String],
        _routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<crate::protocol::tracks::SearchResult> {
        let q = self
            .search_prefixes()
            .into_iter()
            .find_map(|p| query.strip_prefix(p))
            .unwrap_or(query);

        let count = self.search_limit.to_string();
        let track_params = [
            ("q", q.to_string()),
            ("count", count.clone()),
            ("sort", "2".to_string()),
        ];
        let album_params = [("q", q.to_string()), ("count", "20".to_string())];
        let artist_params = [("q", q.to_string()), ("count", "20".to_string())];
        let playlist_params = [("q", q.to_string()), ("count", "20".to_string())];

        let (tracks_resp, albums_resp, artists_resp, playlists_resp) = tokio::join!(
            self.api.call("audio.search", &track_params),
            self.api.call("audio.searchAlbums", &album_params),
            self.api.call("audio.searchArtists", &artist_params),
            self.api.call("audio.searchPlaylists", &playlist_params),
        );

        let tracks: Vec<Track> = tracks_resp
            .as_ref()
            .and_then(|r| r["items"].as_array())
            .map(|arr| arr.iter().filter_map(|i| self.build_track(i)).collect())
            .unwrap_or_default();

        let albums: Vec<PlaylistData> = albums_resp
            .as_ref()
            .and_then(|r| r["items"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|i| self.build_playlist_entry(i, "album"))
                    .collect()
            })
            .unwrap_or_default();

        let artists: Vec<PlaylistData> = artists_resp
            .as_ref()
            .and_then(|r| r["items"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|i| self.build_artist_entry(i))
                    .collect()
            })
            .unwrap_or_default();

        let playlists: Vec<PlaylistData> = playlists_resp
            .as_ref()
            .and_then(|r| r["items"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|i| self.build_playlist_entry(i, "playlist"))
                    .collect()
            })
            .unwrap_or_default();

        Some(crate::protocol::tracks::SearchResult {
            tracks,
            albums,
            artists,
            playlists,
            texts: Vec::new(),
            plugin: json!({}),
        })
    }

    async fn get_track(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<BoxedTrack> {
        let audio_id = if let Some(caps) = self.track_re.captures(identifier) {
            let owner = caps.name("owner")?.as_str();
            let id = caps.name("id")?.as_str();
            format!("{}_{}", owner, id)
        } else {
            identifier.to_string()
        };

        let stream_url = self.resolve_stream_url(&audio_id, None).await?;

        Some(Arc::new(VkMusicTrack {
            stream_url,
            proxy: self.proxy.clone(),
            local_addr: routeplanner.and_then(|rp| rp.get_address()),
        }))
    }

    fn get_proxy_config(&self) -> Option<crate::config::HttpProxyConfig> {
        self.proxy.clone()
    }
}
