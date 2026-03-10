use std::{net::IpAddr, sync::Arc};

use tracing::{debug, error, info, warn};

use crate::{
    audio::{AudioFrame, processor::DecoderCommand},
    config::HttpProxyConfig,
    sources::{
        plugin::{DecoderOutput, PlayableTrack},
        youtube::{
            cipher::YouTubeCipherManager,
            clients::YouTubeClient,
            oauth::YouTubeOAuth,
            utils::{create_reader, detect_audio_kind},
        },
    },
};

pub struct YoutubeTrack {
    pub identifier: String,
    pub clients: Vec<Arc<dyn YouTubeClient>>,
    pub oauth: Arc<YouTubeOAuth>,
    pub cipher_manager: Arc<YouTubeCipherManager>,
    pub visitor_data: Option<String>,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

impl PlayableTrack for YoutubeTrack {
    fn start_decoding(&self, config: crate::config::player::PlayerConfig) -> DecoderOutput {
        let (tx, rx) = flume::bounded::<AudioFrame>((config.buffer_duration_ms / 20) as usize);
        let (cmd_tx, cmd_rx) = flume::bounded(8);
        let (err_tx, err_rx) = flume::bounded(1);

        let identifier_async = self.identifier.clone();
        let cipher_manager_async = self.cipher_manager.clone();
        let oauth_async = self.oauth.clone();
        let clients_async = self.clients.clone();
        let visitor_data_for_task = self.visitor_data.clone();
        let proxy_bg = self.proxy.clone();
        let local_addr_bg = self.local_addr;

        tokio::spawn(async move {
            let context = serde_json::json!({ "visitorData": visitor_data_for_task });

            let mut current_seek_ms = 0u64;
            let mut current_client_index = 0;

            'playback_loop: loop {
                // Resolve a playback URL using any available client.
                let mut resolved_url: Option<(String, String)> = None;

                for (idx, client) in clients_async.iter().enumerate().skip(current_client_index) {
                    current_client_index = idx;
                    let client_name = client.name().to_string();

                    debug!(
                        "YoutubeTrack: Resolving '{}' using {}",
                        identifier_async, client_name
                    );
                    match client
                        .get_track_url(
                            &identifier_async,
                            &context,
                            cipher_manager_async.clone(),
                            oauth_async.clone(),
                        )
                        .await
                    {
                        Ok(Some(url)) => {
                            info!(
                                "YoutubeTrack: resolved track URL for '{}' using client '{}'",
                                identifier_async, client_name
                            );
                            resolved_url = Some((url, client_name));
                            break;
                        }
                        Ok(None) => {
                            debug!(
                                "YoutubeTrack: client {} returned no URL for {}",
                                client_name, identifier_async
                            );
                        }
                        Err(e) => {
                            warn!(
                                "YoutubeTrack: client {} failed to resolve {}: {}",
                                client_name, identifier_async, e
                            );
                        }
                    }
                }

                let (url, client_name) = match resolved_url {
                    Some(r) => r,
                    None => {
                        let msg = format!(
                            "YoutubeTrack: All clients failed to resolve '{}'",
                            identifier_async
                        );
                        error!("{}", msg);
                        let _ = err_tx.send(msg);
                        return;
                    }
                };

                let is_hls = url.contains(".m3u8") || url.contains("/playlist");
                let url_clone = url.clone();
                let cipher_clone = cipher_manager_async.clone();
                let proxy_clone = proxy_bg.clone();
                let client_name_inner = client_name.clone();

                let reader_res = tokio::task::spawn_blocking(move || {
                    create_reader(
                        &url_clone,
                        &client_name_inner,
                        local_addr_bg,
                        proxy_clone,
                        cipher_clone,
                    )
                })
                .await
                .expect("YoutubeTrack: reader spawn_blocking failed");

                let reader = match reader_res {
                    Ok(r) => r,
                    Err(e) => {
                        error!("YoutubeTrack: Reader initialization failed: {}", e);
                        let _ = err_tx.send(e.to_string());
                        return;
                    }
                };

                let kind = detect_audio_kind(&url, is_hls);

                let (inner_cmd_tx, inner_cmd_rx) = flume::bounded(8);
                let tx_clone = tx.clone();
                let err_tx_clone = err_tx.clone();

                let config_for_processor = config.clone();
                let (done_tx, mut done_rx) = tokio::sync::oneshot::channel::<Result<(), String>>();
                let identifier_for_thread = identifier_async.clone();

                std::thread::Builder::new()
                    .name(format!("youtube-decoder-{}", identifier_async))
                    .spawn(move || {
                        let result = match crate::audio::processor::AudioProcessor::new(
                            reader,
                            Some(kind),
                            tx_clone,
                            inner_cmd_rx,
                            Some(err_tx_clone.clone()),
                            config_for_processor,
                        ) {
                            Ok(mut processor) => processor.run().map_err(|e| e.to_string()),
                            Err(e) => {
                                error!(
                                    "YoutubeTrack: AudioProcessor initialization failed for {}: {}",
                                    identifier_for_thread, e
                                );
                                Err(format!("Failed to initialize processor: {}", e))
                            }
                        };
                        let _ = done_tx.send(result);
                    })
                    .expect("failed to spawn youtube decoder thread");

                if current_seek_ms > 0 {
                    let _ = inner_cmd_tx.send(DecoderCommand::Seek(current_seek_ms));
                }

                loop {
                    tokio::select! {
                        cmd_res = cmd_rx.recv_async() => {
                            match cmd_res {
                                Ok(DecoderCommand::Seek(ms)) => {
                                    current_seek_ms = ms;
                                    let _ = inner_cmd_tx.send(DecoderCommand::Seek(ms));
                                }
                                Ok(DecoderCommand::Stop) | Err(_) => {
                                    let _ = inner_cmd_tx.send(DecoderCommand::Stop);
                                    return;
                                }
                            }
                        }
                        res = &mut done_rx => {
                            let res = match res {
                                Ok(r) => Ok(r),
                                Err(_) => Err("decoder thread dropped".to_string()),
                            };
                            match res {
                                Ok(Err(e)) => {
                                    warn!(
                                        "YoutubeTrack: Playback failed for '{}' with client {}: {}. Attempting fallback...",
                                        identifier_async, clients_async[current_client_index].name(), e
                                    );
                                    current_client_index += 1;
                                    if current_client_index < clients_async.len() {
                                        continue 'playback_loop;
                                    } else {
                                        error!(
                                            "YoutubeTrack: All clients failed for '{}'",
                                            identifier_async
                                        );
                                        let _ = err_tx.send(format!("All clients failed: {}", e));
                                    }
                                }
                                Ok(Ok(())) => {
                                    debug!(
                                        "YoutubeTrack: Playback finished for '{}'",
                                        identifier_async
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "YoutubeTrack: Join error for '{}': {}",
                                        identifier_async, e
                                    );
                                }
                            }
                            return;
                        }
                    }
                }
            }
        });

        (rx, cmd_tx, err_rx)
    }
}
