use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use base64::{Engine as _, engine::general_purpose};
use tracing::{debug, warn};

use super::{
    client::TidalClient,
    model::{Manifest, PlaybackInfo},
    track::TidalTrack,
};
use crate::{
    common::types::AudioFormat,
    protocol::tracks::{LoadResult, PlaylistData, PlaylistInfo, Track, TrackInfo},
    sources::playable_track::BoxedTrack,
};

pub struct TidalHifiClient {
    pub quality: String,
    client: Arc<reqwest::Client>,
    tidal_client: Arc<TidalClient>,
    urls: Vec<String>,
    idx: AtomicUsize,
}

impl TidalHifiClient {
    pub fn new(
        client: Arc<reqwest::Client>,
        tidal_client: Arc<TidalClient>,
        urls: Vec<String>,
        quality: String,
    ) -> Self {
        Self {
            quality,
            client,
            tidal_client,
            urls,
            idx: AtomicUsize::new(0),
        }
    }

    fn base_url(&self) -> String {
        let i = self.idx.fetch_add(1, Ordering::Relaxed) % self.urls.len();
        self.urls[i].trim_end_matches('/').to_owned()
    }

    async fn get(&self, path: &str) -> Option<serde_json::Value> {
        let url = format!("{}{}", self.base_url(), path);
        debug!("HiFi: GET {}", url);
        let resp = self.client.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            debug!("HiFi: {} {}", resp.status(), resp.text().await.unwrap_or_default());
            return None;
        }
        resp.json().await.ok()
    }

    fn parse_track(&self, item: &serde_json::Value) -> Option<TrackInfo> {
        let id = item.get("id")?.as_u64()?.to_string();
        let title = item
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Title")
            .to_string();

        let artists = item
            .get("artists")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.get("name").and_then(|n| n.as_str()))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .or_else(|| {
                item.get("artist")
                    .and_then(|a| a.get("name"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
            })
            .unwrap_or_else(|| "Unknown Artist".to_owned());

        let length = item.get("duration").and_then(|v| v.as_u64()).unwrap_or(0) * 1000;

        let isrc = item
            .get("isrc")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned());

        let artwork_url = item
            .get("album")
            .and_then(|a| a.get("cover"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| {
                format!(
                    "https://resources.tidal.com/images/{}/1280x1280.jpg",
                    s.replace("-", "/")
                )
            });

        let url = item
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.replace("http://", "https://"));

        Some(TrackInfo {
            title,
            author: artists,
            length,
            identifier: id,
            is_stream: false,
            uri: url,
            artwork_url,
            isrc,
            source_name: "tidal".to_owned(),
            is_seekable: true,
            position: 0,
        })
    }

    pub async fn load_track(&self, id: &str) -> LoadResult {
        let data = match self.get(&format!("/info/?id={}", id)).await {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };
        let track_obj = data.get("data").unwrap_or(&data);
        self.parse_track(track_obj)
            .map(|i| LoadResult::Track(Track::new(i)))
            .unwrap_or(LoadResult::Empty {})
    }

    pub async fn load_album(&self, id: &str, limit: usize) -> LoadResult {
        let data = match self
            .get(&format!("/album/?id={}&limit={}", id, limit.clamp(1, 500)))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let album = data.get("data").unwrap_or(&data);
        let title = album
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_owned();
        let total = album
            .get("numberOfTracks")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let mut tracks = Vec::new();
        if let Some(items) = album.get("items").and_then(|v| v.as_array()) {
            for item in items {
                let track_obj = item.get("item").unwrap_or(item);
                if let Some(info) = self.parse_track(track_obj) {
                    tracks.push(Track::new(info));
                }
            }
        }

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo { name: title, selected_track: -1 },
            plugin_info: serde_json::json!({
                "type": "album",
                "url": format!("https://tidal.com/browse/album/{}", id),
                "totalTracks": total
            }),
            tracks,
        })
    }

    pub async fn load_playlist(&self, id: &str, limit: usize) -> LoadResult {
        let data = match self
            .get(&format!("/playlist/?id={}&limit={}", id, limit.clamp(1, 500)))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let playlist = data.get("playlist").unwrap_or(&data);
        let title = playlist
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_owned();
        let total = playlist
            .get("numberOfTracks")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let mut tracks = Vec::new();
        if let Some(items) = data.get("items").and_then(|v| v.as_array()) {
            for item in items {
                let track_obj = item.get("item").unwrap_or(item);
                if let Some(info) = self.parse_track(track_obj) {
                    tracks.push(Track::new(info));
                }
            }
        }

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo { name: title, selected_track: -1 },
            plugin_info: serde_json::json!({
                "type": "playlist",
                "url": format!("https://tidal.com/browse/playlist/{}", id),
                "totalTracks": total
            }),
            tracks,
        })
    }

    pub async fn load_mix(&self, id: &str, name_override: Option<String>) -> LoadResult {
        let data = match self.get(&format!("/mix/?id={}", id)).await {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let name = name_override
            .or_else(|| {
                data.get("mix")
                    .and_then(|m| m.get("title"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_owned())
            })
            .unwrap_or_else(|| format!("Mix: {}", id));

        let mut tracks = Vec::new();
        if let Some(items) = data.get("items").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(info) = self.parse_track(item) {
                    tracks.push(Track::new(info));
                }
            }
        }

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo { name, selected_track: -1 },
            plugin_info: serde_json::json!({
                "type": "playlist",
                "url": format!("https://tidal.com/browse/mix/{}", id),
                "totalTracks": tracks.len()
            }),
            tracks,
        })
    }

    pub async fn load_recommendations(&self, id: &str) -> LoadResult {
        let data = match self
            .get(&format!("/recommendations/?id={}", id))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let inner = data.get("data").unwrap_or(&data);
        let mut tracks = Vec::new();
        if let Some(items) = inner.get("items").and_then(|v| v.as_array()) {
            for item in items {
                let track_obj = item.get("track").unwrap_or(item);
                if let Some(info) = self.parse_track(track_obj) {
                    tracks.push(Track::new(info));
                }
            }
        }

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: "Tidal Recommendations".to_owned(),
                selected_track: -1,
            },
            plugin_info: serde_json::json!({
                "type": "playlist",
                "url": format!("https://tidal.com/browse/track/{}", id),
                "totalTracks": tracks.len()
            }),
            tracks,
        })
    }

    pub async fn load_artist_top_tracks(&self, id: &str) -> LoadResult {
        let data = match self
            .get(&format!("/artist/?f={}&skip_tracks=true", id))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let name = data
            .get("albums")
            .and_then(|a| a.get("items"))
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("artists"))
            .and_then(|a| a.as_array())
            .and_then(|arr| arr.first())
            .and_then(|a| a.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Artist")
            .to_owned();

        let mut tracks = Vec::new();
        if let Some(items) = data.get("tracks").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(info) = self.parse_track(item) {
                    tracks.push(Track::new(info));
                }
            }
        }

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: format!("{}'s Top Tracks", name),
                selected_track: -1,
            },
            plugin_info: serde_json::json!({
                "type": "artist",
                "url": format!("https://tidal.com/browse/artist/{}", id),
                "totalTracks": tracks.len()
            }),
            tracks,
        })
    }

    pub async fn search(&self, query: &str) -> LoadResult {
        let encoded = urlencoding::encode(query);
        let data = match self
            .get(&format!("/search/?s={}&limit=10", encoded))
            .await
        {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let inner = data.get("data").unwrap_or(&data);
        let mut tracks = Vec::new();
        if let Some(items) = inner.get("items").and_then(|v| v.as_array()) {
            for item in items {
                if let Some(info) = self.parse_track(item) {
                    tracks.push(Track::new(info));
                }
            }
        }

        if tracks.is_empty() {
            LoadResult::Empty {}
        } else {
            LoadResult::Search(tracks)
        }
    }

    pub async fn get_playback_track(&self, id: &str) -> Option<BoxedTrack> {
        const FALLBACK_CHAIN: &[&str] = &["LOSSLESS", "HIGH", "LOW"];

        let start = match self.quality.as_str() {
            "HI_RES_LOSSLESS" | "LOSSLESS" => 0,
            "HIGH" => 1,
            _ => 2,
        };

        for &quality in &FALLBACK_CHAIN[start..] {
            let data = match self
                .get(&format!("/track/?id={}&quality={}", id, quality))
                .await
            {
                Some(d) => d,
                None => continue,
            };

            let info_val = data.get("data").unwrap_or(&data);
            let info: PlaybackInfo = match serde_json::from_value(info_val.clone()) {
                Ok(i) => i,
                Err(e) => {
                    warn!("HiFi: Failed to parse playback info for {} at {}: {}", id, quality, e);
                    continue;
                }
            };

            if info.manifest_mime_type == "application/dash+xml" {
                debug!("HiFi: track {} returned DASH at {}; skipping", id, quality);
                continue;
            }

            let decoded = match general_purpose::STANDARD.decode(&info.manifest) {
                Ok(d) => d,
                Err(e) => {
                    warn!("HiFi: Failed to decode manifest for {} at {}: {}", id, quality, e);
                    continue;
                }
            };

            let manifest: Manifest = match serde_json::from_slice(&decoded) {
                Ok(m) => m,
                Err(e) => {
                    warn!("HiFi: Failed to parse manifest JSON for {} at {}: {}", id, quality, e);
                    continue;
                }
            };

            let stream_url = match manifest.urls.into_iter().next() {
                Some(u) => u,
                None => {
                    warn!("HiFi: No stream URL in manifest for track {} at {}", id, quality);
                    continue;
                }
            };

            if quality != self.quality.as_str() {
                debug!("HiFi: track {} serving at {} (configured: {})", id, quality, self.quality);
            }

            let mut kind = AudioFormat::from_url(&stream_url);
            if kind == AudioFormat::Unknown {
                kind = match quality {
                    "LOSSLESS" => AudioFormat::Mp4,
                    _ => AudioFormat::Aac,
                };
            }

            return Some(Arc::new(TidalTrack {
                identifier: id.to_owned(),
                stream_url,
                kind,
                client: self.tidal_client.clone(),
            }));
        }

        warn!("HiFi: No playable quality found for track {}", id);
        None
    }
}