use std::sync::Arc;

use serde_json::{Value, json};

use super::{
    YouTubeClient,
    common::{
        INNERTUBE_API, PlayerRequestOptions, make_player_request, resolve_format_url,
        select_best_audio_format,
    },
};
use crate::{
    common::types::AnyResult,
    protocol::tracks::Track,
    sources::youtube::{
        cipher::YouTubeCipherManager,
        clients::common::ClientConfig,
        extractor::{extract_from_player, extract_track},
        oauth::YouTubeOAuth,
    },
};

pub fn extract_visitor_data(context: &Value) -> Option<&str> {
    context
        .get("client")
        .and_then(|c| c.get("visitorData"))
        .and_then(|v| v.as_str())
        .or_else(|| context.get("visitorData").and_then(|v| v.as_str()))
}

pub async fn standard_search<T: YouTubeClient>(
    client: &T,
    http: &Arc<reqwest::Client>,
    query: &str,
    context: &Value,
    _oauth: Arc<YouTubeOAuth>,
    config_builder: impl FnOnce() -> ClientConfig<'static>,
) -> AnyResult<Vec<Track>> {
    let visitor_data = extract_visitor_data(context);
    let config = config_builder();

    let body = json!({
        "context": config.build_context(visitor_data),
        "query": query,
        "params": "EgIQAQ%3D%3D"
    });

    let url = format!("{}/youtubei/v1/search", INNERTUBE_API);

    let mut req = http
        .post(&url)
        .header("User-Agent", client.user_agent())
        .header("X-Goog-Api-Format-Version", "2")
        .header("X-YouTube-Client-Name", client.client_name())
        .header("X-YouTube-Client-Version", client.client_version());

    if let Some(vd) = visitor_data {
        req = req.header("X-Goog-Visitor-Id", vd);
    }

    let res = req.json(&body).send().await?;
    let status = res.status();
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(format!("{} search failed: {} - {}", client.name(), status, text).into());
    }

    let response: Value = res.json().await?;
    let mut tracks = Vec::new();

    if let Some(contents) = response.get("contents") {
        let sections = contents
            .get("sectionListRenderer")
            .and_then(|s| s.get("contents"))
            .and_then(|c| c.as_array())
            .or_else(|| {
                contents
                    .get("twoColumnSearchResultsRenderer")
                    .and_then(|t| t.get("primaryContents"))
                    .and_then(|p| p.get("sectionListRenderer"))
                    .and_then(|s| s.get("contents"))
                    .and_then(|c| c.as_array())
            });

        if let Some(sections) = sections {
            for section in sections {
                let items_opt = section
                    .get("itemSectionRenderer")
                    .and_then(|i| i.get("contents"))
                    .and_then(|c| c.as_array());

                let shelf_items_opt = items_opt
                    .is_none()
                    .then(|| {
                        let shelf = section
                            .get("shelfRenderer")
                            .or_else(|| section.get("richShelfRenderer"))
                            .or_else(|| section.get("reelShelfRenderer"));
                        shelf.and_then(|s| {
                            s.get("content")
                                .and_then(|c| {
                                    c.get("verticalListRenderer")
                                        .or_else(|| c.get("horizontalListRenderer"))
                                })
                                .and_then(|v| v.get("items"))
                                .or_else(|| {
                                    s.get("content")
                                        .and_then(|c| c.get("richGridRenderer"))
                                        .and_then(|r| r.get("contents"))
                                })
                                .and_then(|c| c.as_array())
                        })
                    })
                    .flatten();

                let items = items_opt.or(shelf_items_opt);

                if let Some(items) = items {
                    for item in items {
                        let inner = item
                            .get("richItemRenderer")
                            .and_then(|r| r.get("content"))
                            .unwrap_or(item);

                        if let Some(track) = extract_track(inner, "youtube") {
                            tracks.push(track);
                        }
                    }
                }
            }
        } else if let Some(contents) = contents
            .get("twoColumnSearchResultsRenderer")
            .and_then(|t| t.get("primaryContents"))
            .and_then(|p| p.get("richGridRenderer"))
            .and_then(|r| r.get("contents"))
            .and_then(|c| c.as_array())
        {
            for item in contents {
                let inner = item
                    .get("richItemRenderer")
                    .and_then(|r| r.get("content"))
                    .unwrap_or(item);
                if let Some(track) = extract_track(inner, "youtube") {
                    tracks.push(track);
                }
            }
        } else {
            tracing::debug!(
                "Search: No standard sections found in response contents. keys: {:?}",
                contents.as_object().map(|o| o.keys().collect::<Vec<_>>())
            );
        }
    } else {
        tracing::debug!(
            "Search: No contents found in response. keys: {:?}",
            response.as_object().map(|o| o.keys().collect::<Vec<_>>())
        );
    }

    Ok(tracks)
}

pub struct StandardPlayerOptions<'a, F>
where
    F: FnOnce() -> ClientConfig<'static>,
{
    pub http: &'a Arc<reqwest::Client>,
    pub track_id: &'a str,
    pub context: &'a Value,
    pub oauth: Arc<YouTubeOAuth>,
    pub signature_timestamp: Option<u32>,
    pub encrypted_host_flags: Option<String>,
    pub config_builder: F,
}

pub async fn standard_get_track_info<T, F>(
    client: &T,
    opts: StandardPlayerOptions<'_, F>,
) -> AnyResult<Option<Track>>
where
    T: YouTubeClient,
    F: FnOnce() -> ClientConfig<'static>,
{
    let visitor_data = extract_visitor_data(opts.context);
    let config = (opts.config_builder)();

    let body = make_player_request(PlayerRequestOptions {
        http: opts.http,
        config: &config,
        video_id: opts.track_id,
        params: None,
        visitor_data,
        signature_timestamp: opts.signature_timestamp,
        auth_header: if client.supports_oauth() {
            opts.oauth.get_auth_header().await
        } else {
            None
        },
        referer: None,
        origin: None,
        po_token: None,
        encrypted_host_flags: opts.encrypted_host_flags,
        attestation_request: None,
        serialized_third_party_embed_config: client.is_embedded(),
    })
    .await?;

    Ok(extract_from_player(&body, "youtube"))
}

pub struct StandardUrlOptions<'a, F>
where
    F: FnOnce() -> ClientConfig<'static>,
{
    pub http: &'a Arc<reqwest::Client>,
    pub track_id: &'a str,
    pub context: &'a Value,
    pub cipher_manager: Arc<YouTubeCipherManager>,
    pub oauth: Arc<YouTubeOAuth>,
    pub signature_timestamp: Option<u32>,
    pub encrypted_host_flags: Option<String>,
    pub config_builder: F,
}

pub async fn standard_get_track_url<T, F>(
    client: &T,
    opts: StandardUrlOptions<'_, F>,
) -> AnyResult<Option<String>>
where
    T: YouTubeClient,
    F: FnOnce() -> ClientConfig<'static>,
{
    let visitor_data = extract_visitor_data(opts.context);
    let config = (opts.config_builder)();

    let body = make_player_request(PlayerRequestOptions {
        http: opts.http,
        config: &config,
        video_id: opts.track_id,
        params: None,
        visitor_data,
        signature_timestamp: opts.signature_timestamp,
        auth_header: if client.supports_oauth() {
            opts.oauth.get_auth_header().await
        } else {
            None
        },
        referer: None,
        origin: None,
        po_token: None,
        encrypted_host_flags: opts.encrypted_host_flags,
        attestation_request: None,
        serialized_third_party_embed_config: client.is_embedded(),
    })
    .await?;

    if let Err(e) = crate::sources::youtube::utils::parse_playability_status(&body) {
        tracing::warn!(
            "{} player: video {} not playable: {}",
            client.name(),
            opts.track_id,
            e
        );
        return Err(e.into());
    }

    let streaming_data = match body.get("streamingData") {
        Some(sd) => sd,
        None => {
            tracing::error!(
                "{} player: no streamingData for {}",
                client.name(),
                opts.track_id
            );
            return Ok(None);
        }
    };

    if let Some(hls) = streaming_data
        .get("hlsManifestUrl")
        .and_then(|v| v.as_str())
    {
        tracing::debug!(
            "{} player: using HLS manifest for {}",
            client.name(),
            opts.track_id
        );
        return Ok(Some(hls.to_string()));
    }

    let adaptive = streaming_data
        .get("adaptiveFormats")
        .and_then(|v| v.as_array());
    let formats = streaming_data.get("formats").and_then(|v| v.as_array());

    let player_page_url = format!("https://www.youtube.com/watch?v={}", opts.track_id);

    if let Some(best) = select_best_audio_format(adaptive, formats) {
        match resolve_format_url(best, &player_page_url, &opts.cipher_manager).await {
            Ok(Some(url)) => {
                return Ok(Some(url));
            }
            Ok(None) => {
                tracing::warn!(
                    "{} player: best format had no resolvable URL for {}",
                    client.name(),
                    opts.track_id
                );
            }
            Err(e) => {
                tracing::error!(
                    "{} player: cipher resolution failed for {}: {}",
                    client.name(),
                    opts.track_id,
                    e
                );
                return Err(e);
            }
        }
    }

    Ok(None)
}

pub async fn standard_get_playlist<F>(
    client: &dyn YouTubeClient,
    http: &reqwest::Client,
    playlist_id: &str,
    context: &Value,
    oauth: Arc<YouTubeOAuth>,
    config_builder: F,
) -> AnyResult<Option<(Vec<Track>, String)>>
where
    F: Fn() -> ClientConfig<'static>,
{
    let visitor_data = extract_visitor_data(context);
    let config = config_builder();

    let browse_body = json!({
        "context": config.build_context(visitor_data),
        "browseId": if playlist_id.starts_with("VL") { playlist_id.to_string() } else { format!("VL{}", playlist_id) },
    });

    let browse_url = "https://www.youtube.com/youtubei/v1/browse?prettyPrint=false";
    let mut browse_req = http
        .post(browse_url)
        .header("User-Agent", client.user_agent())
        .header("X-YouTube-Client-Name", client.client_name())
        .header("X-YouTube-Client-Version", client.client_version());

    if let Some(auth) = oauth.get_auth_header().await {
        browse_req = browse_req.header("Authorization", auth);
    }

    if let Some(vd) = visitor_data {
        browse_req = browse_req.header("X-Goog-Visitor-Id", vd);
    }

    if let Ok(res) = browse_req.json(&browse_body).send().await {
        if res.status().is_success() {
            let body: Value = res.json().await?;
            if let Some(result) =
                crate::sources::youtube::extractor::extract_from_browse(&body, "youtube")
            {
                return Ok(Some(result));
            }
        }
    }

    let next_body = json!({
        "context": config.build_context(visitor_data),
        "playlistId": playlist_id,
        "enablePersistentPlaylistPanel": true,
    });

    let next_url = "https://www.youtube.com/youtubei/v1/next?prettyPrint=false";
    let mut next_req = http
        .post(next_url)
        .header("User-Agent", client.user_agent())
        .header("X-YouTube-Client-Name", client.client_name())
        .header("X-YouTube-Client-Version", client.client_version());

    if let Some(auth) = oauth.get_auth_header().await {
        next_req = next_req.header("Authorization", auth);
    }

    if let Some(vd) = visitor_data {
        next_req = next_req.header("X-Goog-Visitor-Id", vd);
    }

    if let Ok(res) = next_req.json(&next_body).send().await {
        if res.status().is_success() {
            let body: Value = res.json().await?;
            if let Some(result) =
                crate::sources::youtube::extractor::extract_from_next(&body, "youtube")
            {
                return Ok(Some(result));
            }
        }
    }

    Ok(None)
}
