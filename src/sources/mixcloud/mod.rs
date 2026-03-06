use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use regex::Regex;
use serde_json::{Value, json};

use crate::{
    protocol::tracks::{LoadResult, PlaylistData, PlaylistInfo, Track, TrackInfo},
    sources::{SourcePlugin, plugin::BoxedTrack},
};

pub mod reader;
pub mod track;

const DECRYPTION_KEY: &[u8] = b"IFYOUWANTTHEARTISTSTOGETPAIDDONOTDOWNLOADFROMMIXCLOUD";
const GRAPHQL_URL: &str = "https://app.mixcloud.com/graphql";

fn track_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)^https?://(?:(?:www|beta|m)\.)?mixcloud\.com/(?P<user>[^/]+)/(?P<slug>[^/]+)/?$",
        )
        .unwrap()
    })
}

fn playlist_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^https?://(?:(?:www|beta|m)\.)?mixcloud\.com/(?P<user>[^/]+)/playlists/(?P<playlist>[^/]+)/?$").unwrap()
    })
}

fn user_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^https?://(?:(?:www|beta|m)\.)?mixcloud\.com/(?P<id>[^/]+)(?:/(?P<type>uploads|favorites|listens|stream))?/?$").unwrap()
    })
}

pub struct MixcloudSource {
    client: Arc<reqwest::Client>,
    search_limit: usize,
}

impl MixcloudSource {
    pub fn new(
        config: Option<crate::config::MixcloudConfig>,
        client: Arc<reqwest::Client>,
    ) -> Result<Self, String> {
        Ok(Self {
            client,
            search_limit: config.map(|c| c.search_limit).unwrap_or(10),
        })
    }
}

pub fn decrypt(ciphertext_b64: &str) -> String {
    let ciphertext: Vec<u8> = match general_purpose::STANDARD.decode(ciphertext_b64) {
        Ok(b) => b,
        Err(_) => return String::new(),
    };

    let mut decrypted = Vec::with_capacity(ciphertext.len());
    for (i, &byte) in ciphertext.iter().enumerate() {
        decrypted.push(byte ^ DECRYPTION_KEY[i % DECRYPTION_KEY.len()]);
    }

    String::from_utf8(decrypted).unwrap_or_default()
}

impl MixcloudSource {
    async fn graphql_request(&self, query: &str) -> Option<Value> {
        let url = format!("{GRAPHQL_URL}?query={}", urlencoding::encode(query));
        let resp = self.client.get(url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        resp.json::<Value>().await.ok()
    }

    fn parse_track_data(&self, data: &Value) -> Option<Track> {
        let url_raw = data["url"].as_str()?;
        let path_parts: Vec<&str> = url_raw
            .split("mixcloud.com/")
            .nth(1)?
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        if path_parts.len() < 2 {
            return None;
        }

        let user_id = path_parts[0];
        let slug = path_parts[1];
        let id = format!("{user_id}_{slug}");
        let title = data["name"].as_str()?.to_owned();
        let author = data["owner"]["displayName"]
            .as_str()
            .unwrap_or(user_id)
            .to_owned();
        let duration_ms = data["audioLength"].as_u64().unwrap_or(0) * 1000;
        let artwork_url = data["picture"]["url"].as_str().map(|s| s.to_owned());

        Some(Track::new(TrackInfo {
            identifier: id,
            is_seekable: true,
            author,
            length: duration_ms,
            is_stream: false,
            position: 0,
            title,
            uri: Some(url_raw.to_owned()),
            artwork_url,
            isrc: None,
            source_name: "mixcloud".to_owned(),
        }))
    }

    async fn resolve_track(&self, username: &str, slug: &str) -> LoadResult {
        let query = format!(
            "{{
                cloudcastLookup(lookup: {{username: \"{username}\", slug: \"{slug}\"}}) {{
                  audioLength
                  name
                  url
                  owner {{ displayName username }}
                  picture(width: 1024, height: 1024) {{ url }}
                  streamInfo {{ hlsUrl url }}
                  restrictedReason
                }}
            }}"
        );

        match self.graphql_request(&query).await {
            Some(body) => {
                if let Some(data) = body["data"]["cloudcastLookup"].as_object() {
                    if let Some(reason) = data.get("restrictedReason").and_then(|v| v.as_str()) {
                        return LoadResult::Error(crate::protocol::tracks::LoadError {
                            message: Some(format!("Track restricted: {reason}")),
                            severity: crate::common::Severity::Common,
                            cause: reason.to_owned(),
                            cause_stack_trace: None,
                        });
                    }

                    if let Some(track) = self.parse_track_data(&Value::Object(data.clone())) {
                        return LoadResult::Track(track);
                    }
                }
                LoadResult::Empty {}
            }
            None => LoadResult::Empty {},
        }
    }

    async fn resolve_playlist(&self, user: &str, slug: &str) -> LoadResult {
        let query_template = |cursor: Option<&str>| {
            let cursor_arg = cursor
                .map(|c| format!(", after: \"{c}\""))
                .unwrap_or_default();
            format!(
                "{{
                    playlistLookup(lookup: {{username: \"{user}\", slug: \"{slug}\"}}) {{
                      name
                      items(first: 100{cursor_arg}) {{
                        edges {{
                          node {{
                            cloudcast {{
                              audioLength
                              name
                              url
                              owner {{ displayName username }}
                              picture(width: 1024, height: 1024) {{ url }}
                              streamInfo {{ hlsUrl url }}
                            }}
                          }}
                        }}
                        pageInfo {{ endCursor hasNextPage }}
                      }}
                    }}
                }}"
            )
        };

        let mut tracks = Vec::new();
        let mut cursor: Option<String> = None;
        let mut playlist_name = "Mixcloud Playlist".to_owned();

        loop {
            let query = query_template(cursor.as_deref());
            let body = match self.graphql_request(&query).await {
                Some(b) => b,
                None => break,
            };

            let lookup = &body["data"]["playlistLookup"];
            if lookup.is_null() {
                break;
            }

            if let Some(name) = lookup["name"].as_str() {
                playlist_name = name.to_owned();
            }

            if let Some(edges) = lookup["items"]["edges"].as_array() {
                for edge in edges {
                    if let Some(track) = self.parse_track_data(&edge["node"]["cloudcast"]) {
                        tracks.push(track);
                    }
                }
            }

            if lookup["items"]["pageInfo"]["hasNextPage"].as_bool() == Some(true) {
                cursor = lookup["items"]["pageInfo"]["endCursor"]
                    .as_str()
                    .map(|s| s.to_owned());
                if cursor.is_none() || tracks.len() >= 1000 {
                    break;
                }
            } else {
                break;
            }
        }

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: playlist_name,
                selected_track: -1,
            },
            plugin_info: json!({}),
            tracks,
        })
    }

    async fn resolve_user(&self, username: &str, list_type: &str) -> LoadResult {
        let (query_type, node_query) = match list_type {
            "stream" => (
                "stream",
                "... on Cloudcast { audioLength name url owner { displayName username } picture(width: 1024, height: 1024) { url } streamInfo { hlsUrl url } }",
            ),
            _ => (
                list_type,
                "audioLength name url owner { displayName username } picture(width: 1024, height: 1024) { url } streamInfo { hlsUrl url }",
            ),
        };

        let query_template = |cursor: Option<&str>| {
            let cursor_arg = cursor
                .map(|c| format!(", after: \"{c}\""))
                .unwrap_or_default();
            format!(
                "{{
                    userLookup(lookup: {{username: \"{username}\"}}) {{
                      displayName
                      {query_type}(first: 100{cursor_arg}) {{
                        edges {{
                          node {{
                            {node_query}
                          }}
                        }}
                        pageInfo {{ endCursor hasNextPage }}
                      }}
                    }}
                }}"
            )
        };

        let mut tracks = Vec::new();
        let mut cursor: Option<String> = None;
        let mut display_name = username.to_owned();

        loop {
            let query = query_template(cursor.as_deref());
            let body = match self.graphql_request(&query).await {
                Some(b) => b,
                None => break,
            };

            let lookup = &body["data"]["userLookup"];
            if lookup.is_null() {
                break;
            }

            display_name = lookup["displayName"]
                .as_str()
                .unwrap_or(username)
                .to_owned();

            if let Some(edges) = lookup[query_type]["edges"].as_array() {
                for edge in edges {
                    if let Some(track) = self.parse_track_data(&edge["node"]) {
                        tracks.push(track);
                    }
                }
            }

            if lookup[query_type]["pageInfo"]["hasNextPage"].as_bool() == Some(true) {
                cursor = lookup[query_type]["pageInfo"]["endCursor"]
                    .as_str()
                    .map(|s| s.to_owned());
                if cursor.is_none() || tracks.len() >= 1000 {
                    break;
                }
            } else {
                break;
            }
        }

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Playlist(PlaylistData {
            info: PlaylistInfo {
                name: format!("{display_name} ({list_type})"),
                selected_track: -1,
            },
            plugin_info: json!({}),
            tracks,
        })
    }

    async fn search(&self, query_raw: &str) -> LoadResult {
        let url = format!(
            "https://api.mixcloud.com/search/?q={}&type=cloudcast",
            urlencoding::encode(query_raw)
        );
        let resp = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(_) => return LoadResult::Empty {},
        };

        let body: Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => return LoadResult::Empty {},
        };

        let mut tracks = Vec::new();
        if let Some(data) = body["data"].as_array() {
            for item in data {
                if let Some(url_raw) = item["url"].as_str() {
                    let path_parts: Vec<&str> = url_raw
                        .split("mixcloud.com/")
                        .nth(1)
                        .unwrap_or("")
                        .split('/')
                        .filter(|s| !s.is_empty())
                        .collect();
                    if path_parts.len() < 2 {
                        continue;
                    }

                    let id = format!("{}_{}", path_parts[0], path_parts[1]);
                    tracks.push(Track::new(TrackInfo {
                        identifier: id,
                        is_seekable: true,
                        author: item["user"]["name"]
                            .as_str()
                            .unwrap_or(path_parts[0])
                            .to_owned(),
                        length: item["audio_length"].as_u64().unwrap_or(0) * 1000,
                        is_stream: false,
                        position: 0,
                        title: item["name"].as_str().unwrap_or("Unknown").to_owned(),
                        uri: Some(url_raw.to_owned()),
                        artwork_url: item["pictures"]["large"]
                            .as_str()
                            .or_else(|| item["pictures"]["medium"].as_str())
                            .map(|s| s.to_owned()),
                        isrc: None,
                        source_name: "mixcloud".to_owned(),
                    }));
                }

                if tracks.len() >= self.search_limit {
                    break;
                }
            }
        }

        if tracks.is_empty() {
            return LoadResult::Empty {};
        }

        LoadResult::Search(tracks)
    }
}

pub async fn fetch_track_stream_info(
    client: &Arc<reqwest::Client>,
    url: &str,
) -> Option<(Option<String>, Option<String>)> {
    let path_parts: Vec<&str> = url
        .split("mixcloud.com/")
        .nth(1)?
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    if path_parts.len() < 2 {
        return None;
    }

    let query = format!(
        "{{
            cloudcastLookup(lookup: {{username: \"{}\", slug: \"{}\"}}) {{
              streamInfo {{ hlsUrl url }}
            }}
        }}",
        path_parts[0], path_parts[1]
    );

    let body = graphql_request_internal(client, &query).await?;
    let data = body["data"]["cloudcastLookup"].as_object()?;

    let info = data.get("streamInfo")?;
    let hls = info
        .get("hlsUrl")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());
    let stream = info
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned());

    Some((hls, stream))
}

#[async_trait]
impl SourcePlugin for MixcloudSource {
    fn name(&self) -> &str {
        "mixcloud"
    }

    fn can_handle(&self, identifier: &str) -> bool {
        identifier.starts_with("mcsearch:")
            || track_url_re().is_match(identifier)
            || playlist_url_re().is_match(identifier)
            || user_url_re().is_match(identifier)
    }

    fn search_prefixes(&self) -> Vec<&str> {
        vec!["mcsearch:"]
    }

    async fn load(
        &self,
        identifier: &str,
        _routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> LoadResult {
        if let Some(query) = identifier.strip_prefix("mcsearch:") {
            return self.search(query).await;
        }

        if let Some(caps) = playlist_url_re().captures(identifier) {
            return self
                .resolve_playlist(&caps["user"], &caps["playlist"])
                .await;
        }

        if let Some(caps) = user_url_re().captures(identifier) {
            return self
                .resolve_user(
                    &caps["id"],
                    caps.name("type").map(|m| m.as_str()).unwrap_or("uploads"),
                )
                .await;
        }

        if let Some(caps) = track_url_re().captures(identifier) {
            return self.resolve_track(&caps["user"], &caps["slug"]).await;
        }

        LoadResult::Empty {}
    }

    async fn get_track(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<BoxedTrack> {
        let url = match self.load(identifier, None).await {
            LoadResult::Track(track) => track.info.uri?,
            _ => return None,
        };

        let (enc_hls, enc_url) = fetch_track_stream_info(&self.client, &url)
            .await
            .unwrap_or((None, None));

        let hls_url = enc_hls.map(|s| decrypt(&s));
        let stream_url = enc_url.map(|s| decrypt(&s));

        if hls_url.is_none() && stream_url.is_none() {
            return None;
        }

        Some(Box::new(track::MixcloudTrack {
            client: self.client.clone(),
            hls_url,
            stream_url,
            uri: url,
            local_addr: routeplanner.and_then(|rp| rp.get_address()),
        }))
    }
}

async fn graphql_request_internal(client: &Arc<reqwest::Client>, query: &str) -> Option<Value> {
    let url = format!("{GRAPHQL_URL}?query={}", urlencoding::encode(query));
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<Value>().await.ok()
}
