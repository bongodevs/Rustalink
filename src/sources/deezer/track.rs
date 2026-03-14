use std::{net::IpAddr, sync::Arc};

use tracing::{debug, error, warn};

use crate::{
    audio::{
        AudioFrame,
        processor::{AudioProcessor, DecoderCommand},
    },
    config::HttpProxyConfig,
    sources::{
        deezer::{reader::DeezerReader, token::DeezerTokenTracker},
        plugin::{DecoderOutput, PlayableTrack},
    },
};

pub struct DeezerTrack {
    pub client: Arc<reqwest::Client>,
    pub track_id: String,
    pub token_tracker: Arc<DeezerTokenTracker>,
    pub master_key: String,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

struct ResolvedUrl {
    cdn_url: String,
    track_id: String,
    arl_index: usize,
}

impl PlayableTrack for DeezerTrack {
    fn start_decoding(&self, config: crate::config::player::PlayerConfig) -> DecoderOutput {
        let (tx, rx) = flume::bounded::<AudioFrame>((config.buffer_duration_ms / 20) as usize);
        let (cmd_tx, cmd_rx) = flume::unbounded::<DecoderCommand>();
        let (err_tx, err_rx) = flume::bounded::<String>(1);

        let track_id = self.track_id.clone();
        let client = self.client.clone();
        let token_tracker = self.token_tracker.clone();
        let master_key = self.master_key.clone();
        let local_addr = self.local_addr;
        let proxy = self.proxy.clone();

        tokio::spawn(async move {
            const MAX_RETRIES: u32 = 3;
            let mut attempt = 0u32;
            let mut last_arl: Option<usize> = None;

            let success = loop {
                if attempt > MAX_RETRIES {
                    break false;
                }

                if let Some(idx) = last_arl.take() {
                    token_tracker.invalidate_token(idx).await;
                }

                let resolved = match resolve_cdn_url(&client, &token_tracker, &track_id).await {
                    Some(r) => r,
                    None => {
                        attempt += 1;
                        continue;
                    }
                };

                last_arl = Some(resolved.arl_index);

                let proxy_for_attempt = proxy.clone();
                let master_key_clone = master_key.clone();
                let cdn_url = resolved.cdn_url.clone();
                let effective_id = resolved.track_id.clone();

                let reader_result = match tokio::task::spawn_blocking(move || {
                    DeezerReader::new(
                        &cdn_url,
                        &effective_id,
                        &master_key_clone,
                        local_addr,
                        proxy_for_attempt,
                    )
                    .map(|r| {
                        (
                            Box::new(r) as Box<dyn symphonia::core::io::MediaSource>,
                            cdn_url,
                        )
                    })
                    .map_err(|e| e.to_string())
                })
                .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        warn!("Deezer: reader spawn_blocking failed for {track_id}: {e}");
                        attempt += 1;
                        continue;
                    }
                };

                let (reader, final_url) = match reader_result {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(
                            "Deezer CDN open failed for {} (attempt {}/{}): {e} — rotating ARL",
                            track_id,
                            attempt + 1,
                            MAX_RETRIES + 1,
                        );
                        attempt += 1;
                        continue;
                    }
                };

                let kind = crate::common::types::AudioFormat::from_url(&final_url);
                let err_tx_setup = err_tx.clone();
                let setup = match tokio::task::spawn_blocking(move || {
                    AudioProcessor::new(reader, Some(kind), tx, cmd_rx, Some(err_tx_setup), config)
                })
                .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        error!("Deezer: processor spawn_blocking failed for {track_id}: {e}");
                        let _ = err_tx.send(format!("Processor spawn failure: {e}"));
                        break false;
                    }
                };

                match setup {
                    Ok(mut processor) => {
                        let id = track_id.clone();
                        if let Err(e) = std::thread::Builder::new()
                            .name(format!("deezer-decoder-{track_id}"))
                            .spawn(move || {
                                if let Err(e) = processor.run() {
                                    error!("Deezer processor error for {id}: {e}");
                                }
                            })
                        {
                            let _ = err_tx.send(format!("Failed to spawn decoder thread: {e}"));
                        }
                    }
                    Err(e) => {
                        error!("Deezer processor init failed for {track_id}: {e}");
                        let _ = err_tx.send(format!("Failed to initialize processor: {e}"));
                    }
                }

                break true;
            };

            if !success {
                error!("Deezer: all retries exhausted for {track_id}");
                let _ = err_tx.send("Failed to open Deezer stream after retries".to_owned());
            }
        });

        (rx, cmd_tx, err_rx)
    }
}

async fn resolve_cdn_url(
    client: &Arc<reqwest::Client>,
    token_tracker: &Arc<DeezerTokenTracker>,
    track_id: &str,
) -> Option<ResolvedUrl> {
    let tokens = token_tracker.get_token().await?;
    let arl_index = tokens.arl_index;

    let song_url = format!(
        "https://www.deezer.com/ajax/gw-light.php?method=song.getData&input=3&api_version=1.0&api_token={}",
        tokens.api_token
    );

    let json: serde_json::Value = match client
        .post(&song_url)
        .header(
            "Cookie",
            format!(
                "sid={}; dzr_uniq_id={}",
                tokens.session_id, tokens.dzr_uniq_id
            ),
        )
        .json(&serde_json::json!({ "sng_id": track_id }))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(v) => v,
            Err(_) => {
                token_tracker.invalidate_token(arl_index).await;
                return None;
            }
        },
        Err(e) => {
            debug!("Deezer: song.getData failed: {e}");
            token_tracker.invalidate_token(arl_index).await;
            return None;
        }
    };

    if json
        .get("error")
        .and_then(|v| v.as_array())
        .is_some_and(|e| !e.is_empty())
    {
        debug!("Deezer: API error in song.getData");
        token_tracker.invalidate_token(arl_index).await;
        return None;
    }

    let mut results = json.get("results")?.clone();

    // If the track has no rights, try the FALLBACK entry.
    let rights = results.get("RIGHTS");
    if is_rights_empty(rights)
        && let Some(fallback) = results.get("FALLBACK")
        && !fallback
            .get("TRACK_TOKEN")
            .map(|v| v.is_null())
            .unwrap_or(true)
    {
        let fallback_id = fallback.get("SNG_ID").and_then(|v| {
            v.as_str()
                .map(|s| s.to_owned())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        });
        if let Some(id) = fallback_id {
            debug!("Deezer: track {track_id} has no RIGHTS, using FALLBACK {id}");
            results = fallback.clone();
            let track_token = results.get("TRACK_TOKEN").and_then(|v| v.as_str())?;
            return fetch_media_url(client, token_tracker, &tokens, track_token, &id, arl_index)
                .await;
        } else {
            warn!("Deezer: track {track_id} FALLBACK SNG_ID has unexpected format");
        }
    }

    let track_token = results.get("TRACK_TOKEN").and_then(|v| v.as_str())?;
    fetch_media_url(
        client,
        token_tracker,
        &tokens,
        track_token,
        track_id,
        arl_index,
    )
    .await
}

async fn fetch_media_url(
    client: &Arc<reqwest::Client>,
    token_tracker: &Arc<DeezerTokenTracker>,
    tokens: &crate::sources::deezer::token::DeezerTokens,
    track_token: &str,
    effective_track_id: &str,
    arl_index: usize,
) -> Option<ResolvedUrl> {
    let body = serde_json::json!({
        "license_token": tokens.license_token,
        "media": [{ "type": "FULL", "formats": [
            { "cipher": "BF_CBC_STRIPE", "format": "MP3_128" },
            { "cipher": "BF_CBC_STRIPE", "format": "MP3_64" }
        ]}],
        "track_tokens": [track_token]
    });

    let json: serde_json::Value = match client
        .post("https://media.deezer.com/v1/get_url")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(v) => v,
            Err(_) => {
                token_tracker.invalidate_token(arl_index).await;
                return None;
            }
        },
        Err(e) => {
            debug!("Deezer: get_url failed: {e}");
            token_tracker.invalidate_token(arl_index).await;
            return None;
        }
    };

    if json
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("errors"))
        .and_then(|e| e.as_array())
        .is_some_and(|e| !e.is_empty())
    {
        debug!("Deezer: get_url returned errors");
        token_tracker.invalidate_token(arl_index).await;
        return None;
    }

    let cdn_url = json
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("media"))
        .and_then(|m| m.get(0))
        .and_then(|m| m.get("sources"))
        .and_then(|s| s.get(0))
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())?
        .to_owned();

    Some(ResolvedUrl {
        cdn_url,
        track_id: effective_track_id.to_owned(),
        arl_index,
    })
}

pub(super) async fn verify_track_resolvable(
    client: &Arc<reqwest::Client>,
    track_id: &str,
    token_tracker: &DeezerTokenTracker,
) -> Option<String> {
    let tokens = token_tracker.get_token().await?;

    let song_url = format!(
        "https://www.deezer.com/ajax/gw-light.php?method=song.getData&input=3&api_version=1.0&api_token={}",
        tokens.api_token
    );
    let json: serde_json::Value = client
        .post(&song_url)
        .header(
            "Cookie",
            format!(
                "sid={}; dzr_uniq_id={}",
                tokens.session_id, tokens.dzr_uniq_id
            ),
        )
        .json(&serde_json::json!({ "sng_id": track_id }))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    if json
        .get("error")
        .and_then(|v| v.as_array())
        .is_some_and(|e| !e.is_empty())
    {
        token_tracker.invalidate_token(tokens.arl_index).await;
        return None;
    }

    let mut results = match json.get("results") {
        Some(r) => r.clone(),
        None => {
            token_tracker.invalidate_token(tokens.arl_index).await;
            return None;
        }
    };

    let rights = results.get("RIGHTS");
    if is_rights_empty(rights)
        && let Some(fallback) = results.get("FALLBACK")
        && !fallback
            .get("TRACK_TOKEN")
            .map(|v| v.is_null())
            .unwrap_or(true)
    {
        let has_id = fallback.get("SNG_ID").and_then(|v| {
            v.as_str()
                .map(|s| s.to_owned())
                .or_else(|| v.as_i64().map(|n| n.to_string()))
        });
        if has_id.is_some() {
            results = fallback.clone();
        } else {
            warn!(
                "Deezer: track {track_id} FALLBACK SNG_ID has unexpected format: {:?}",
                fallback.get("SNG_ID")
            );
        }
    }

    let track_token = results
        .get("TRACK_TOKEN")
        .and_then(|v| v.as_str())?
        .to_owned();

    let media_json: serde_json::Value = client
        .post("https://media.deezer.com/v1/get_url")
        .json(&serde_json::json!({
            "license_token": tokens.license_token,
            "media": [{ "type": "FULL", "formats": [
                { "cipher": "BF_CBC_STRIPE", "format": "MP3_128" }
            ]}],
            "track_tokens": [track_token]
        }))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    if media_json
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("errors"))
        .and_then(|e| e.as_array())
        .is_some_and(|e| !e.is_empty())
    {
        token_tracker.invalidate_token(tokens.arl_index).await;
        return None;
    }

    media_json
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("media"))
        .and_then(|m| m.get(0))
        .and_then(|m| m.get("sources"))
        .and_then(|s| s.get(0))
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .map(|s| s.to_owned())
}

fn is_rights_empty(rights: Option<&serde_json::Value>) -> bool {
    rights
        .map(|v| {
            v.as_array()
                .map(|a| a.is_empty())
                .or_else(|| v.as_object().map(|o| o.is_empty()))
                .unwrap_or(true)
        })
        .unwrap_or(true)
}
