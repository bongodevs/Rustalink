use std::{collections::BTreeMap, net::IpAddr, sync::Arc};

use rand::{Rng, distributions::Alphanumeric, thread_rng};

use crate::{
    audio::{AudioFrame, processor::DecoderCommand},
    sources::{
        audiomack::utils::build_auth_header,
        http::HttpTrack,
        plugin::{DecoderOutput, PlayableTrack},
    },
};

pub struct AudiomackTrack {
    pub client: Arc<reqwest::Client>,
    pub identifier: String,
    pub local_addr: Option<IpAddr>,
}

impl PlayableTrack for AudiomackTrack {
    fn start_decoding(&self, config: crate::config::player::PlayerConfig) -> DecoderOutput {
        let (tx, rx) = flume::bounded::<AudioFrame>((config.buffer_duration_ms / 20) as usize);
        let (cmd_tx, cmd_rx) = flume::unbounded::<DecoderCommand>();
        let (err_tx, err_rx) = flume::bounded::<String>(1);

        let identifier = self.identifier.clone();
        let client = self.client.clone();
        let local_addr = self.local_addr;

        let handle = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            let _guard = handle.enter();
            handle.block_on(async move {
                if let Some(url) = fetch_stream_url(&client, &identifier).await {
                    let http_track = HttpTrack {
                        url,
                        local_addr,
                        proxy: None,
                    };
                    let (inner_rx, inner_cmd_tx, inner_err_rx) =
                        http_track.start_decoding(config.clone());

                    // Proxy commands
                    let cmd_tx_clone = inner_cmd_tx.clone();
                    std::thread::spawn(move || {
                        while let Ok(cmd) = cmd_rx.recv() {
                            let _ = cmd_tx_clone.send(cmd);
                        }
                    });

                    // Proxy errors
                    let err_tx_clone = err_tx.clone();
                    std::thread::spawn(move || {
                        while let Ok(err) = inner_err_rx.recv() {
                            let _ = err_tx_clone.send(err);
                        }
                    });

                    // Proxy samples
                    while let Ok(sample) = inner_rx.recv() {
                        if tx.send(sample).is_err() {
                            break;
                        }
                    }
                } else {
                    let _ = err_tx.send("Failed to fetch Audiomack stream URL".to_owned());
                }
            });
        });

        (rx, cmd_tx, err_rx)
    }
}

async fn fetch_stream_url(client: &Arc<reqwest::Client>, identifier: &str) -> Option<String> {
    let nonce = generate_nonce();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    // Strategy 1: POST /music/{id}/play (preferred for web)
    let post_url = format!("https://api.audiomack.com/v1/music/{identifier}/play");
    let mut body = BTreeMap::new();
    body.insert("environment".to_owned(), "desktop-web".to_owned());
    body.insert("session".to_owned(), "backend-session".to_owned());
    body.insert("hq".to_owned(), "true".to_owned());

    let auth_post = build_auth_header("POST", &post_url, &body, &nonce, &timestamp);
    if let Ok(resp) = client
        .post(&post_url)
        .header("Authorization", auth_post)
        .form(&body)
        .send()
        .await
        && let Some(url) = parse_response(resp).await
    {
        return Some(url);
    }

    // Strategy 2: GET /music/play/{id} (legacy/fallback)
    let get_url = format!("https://api.audiomack.com/v1/music/play/{identifier}");
    let mut query = BTreeMap::new();
    query.insert("environment".to_owned(), "desktop-web".to_owned());
    query.insert("hq".to_owned(), "true".to_owned());

    let auth_get = build_auth_header("GET", &get_url, &query, &nonce, &timestamp);
    if let Ok(resp) = client
        .get(&get_url)
        .header("Authorization", auth_get)
        .query(&query)
        .send()
        .await
        && let Some(url) = parse_response(resp).await
    {
        return Some(url);
    }

    None
}

async fn parse_response(resp: reqwest::Response) -> Option<String> {
    if !resp.status().is_success() {
        return None;
    }

    let text = resp.text().await.ok()?;
    if text.starts_with("http") {
        return Some(text);
    }

    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    if let Some(s) = json.as_str() {
        return Some(s.to_owned());
    }

    let results = json.get("results").unwrap_or(&json);
    results
        .get("signedUrl")
        .or_else(|| results.get("signed_url"))
        .or_else(|| results.get("url"))
        .or_else(|| results.get("streamUrl"))
        .or_else(|| results.get("stream_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
}

fn generate_nonce() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}
