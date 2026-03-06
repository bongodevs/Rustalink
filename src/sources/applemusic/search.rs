use std::collections::HashSet;

use serde_json::Value;

use super::{API_BASE, AppleMusicSource};
use crate::protocol::tracks::{LoadResult, PlaylistData, PlaylistInfo, SearchResult};

impl AppleMusicSource {
    pub(crate) async fn search(&self, query: &str) -> LoadResult {
        let encoded_query = urlencoding::encode(query);
        let path = format!(
            "/catalog/{}/search?term={}&limit=10&types=songs",
            self.country_code, encoded_query
        );

        let data = match self.api_request(&path).await {
            Some(d) => d,
            None => return LoadResult::Empty {},
        };

        let songs = data
            .pointer("/results/songs/data")
            .and_then(|v| v.as_array());

        let mut tracks = Vec::new();
        if let Some(items) = songs {
            for item in items {
                if let Some(track) = self.build_track(item, None) {
                    tracks.push(track);
                }
            }
        }

        if tracks.is_empty() {
            LoadResult::Empty {}
        } else {
            LoadResult::Search(tracks)
        }
    }

    pub(crate) async fn get_search_suggestions(
        &self,
        query: &str,
        types: &[String],
    ) -> Option<SearchResult> {
        let mut kinds = HashSet::new();
        let mut am_types = Vec::new();
        let all_types = types.is_empty();

        if all_types
            || types.contains(&"track".to_owned())
            || types.contains(&"album".to_owned())
            || types.contains(&"artist".to_owned())
            || types.contains(&"playlist".to_owned())
        {
            kinds.insert("topResults");
        }

        if types.contains(&"text".to_owned()) {
            kinds.insert("terms");
        }

        if all_types || types.contains(&"track".to_owned()) {
            am_types.push("songs");
        }
        if all_types || types.contains(&"album".to_owned()) {
            am_types.push("albums");
        }
        if all_types || types.contains(&"artist".to_owned()) {
            am_types.push("artists");
        }
        if all_types || types.contains(&"playlist".to_owned()) {
            am_types.push("playlists");
        }

        let kinds_str = kinds.into_iter().collect::<Vec<_>>().join(",");
        let types_str = am_types.join(",");

        let mut params = vec![
            ("term", query.to_owned()),
            ("extend", "artistUrl".to_owned()),
            ("kinds", kinds_str),
        ];

        if !types_str.is_empty() {
            params.push(("types", types_str));
        }

        let path = format!("/catalog/{}/search/suggestions", self.country_code);
        let mut url = format!("{}{}", API_BASE, path);
        if !params.is_empty() {
            url.push('?');
            for (i, (k, v)) in params.iter().enumerate() {
                if i > 0 {
                    url.push('&');
                }
                url.push_str(k);
                url.push('=');
                url.push_str(&urlencoding::encode(v));
            }
        }

        let json = self.api_request(&url).await?;
        let suggestions = json.pointer("/results/suggestions")?.as_array()?;

        let mut tracks = Vec::new();
        let mut albums = Vec::new();
        let mut artists = Vec::new();
        let mut playlists = Vec::new();

        for suggestion in suggestions {
            let kind = suggestion
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if kind == "terms" {
                continue;
            }

            let content = match suggestion.get("content") {
                Some(c) => c,
                None => continue,
            };

            let type_ = content.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match type_ {
                "songs" => {
                    if let Some(track) = self.build_track(content, None) {
                        tracks.push(track);
                    }
                }
                "albums" => {
                    if let Some(album) = self.build_collection(content, "album") {
                        albums.push(album);
                    }
                }
                "artists" => {
                    if let Some(artist) = self.build_collection(content, "artist") {
                        artists.push(artist);
                    }
                }
                "playlists" => {
                    if let Some(playlist) = self.build_collection(content, "playlist") {
                        playlists.push(playlist);
                    }
                }
                _ => {}
            }
        }

        Some(SearchResult {
            tracks,
            albums,
            artists,
            playlists,
            texts: Vec::new(),
            plugin: serde_json::json!({}),
        })
    }

    fn build_collection(&self, content: &Value, kind: &str) -> Option<PlaylistData> {
        let attributes = content.get("attributes")?;
        let url = attributes.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let name = attributes
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        let artwork = attributes
            .pointer("/artwork/url")
            .and_then(|v| v.as_str())
            .map(|s| s.replace("{w}", "500").replace("{h}", "500"));

        let (author, track_count, display_name) = match kind {
            "album" => (
                attributes
                    .get("artistName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown Artist")
                    .to_owned(),
                attributes
                    .get("trackCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                name.to_owned(),
            ),
            "artist" => (name.to_owned(), 0, format!("{}'s Top Tracks", name)),
            "playlist" => (
                attributes
                    .get("curatorName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Apple Music")
                    .to_owned(),
                attributes
                    .get("trackCount")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                name.to_owned(),
            ),
            _ => return None,
        };

        Some(PlaylistData {
            info: PlaylistInfo {
                name: display_name,
                selected_track: -1,
            },
            plugin_info: serde_json::json!({
                "type": kind,
                "url": url,
                "author": author,
                "artworkUrl": artwork,
                "totalTracks": track_count
            }),
            tracks: Vec::new(),
        })
    }
}
