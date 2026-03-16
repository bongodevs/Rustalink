use std::sync::Arc;

use async_trait::async_trait;
use md5::{Digest, Md5};

use crate::{
    common::types::AnyResult,
    protocol::tracks::TrackInfo,
    sources::{
        http::HttpTrack,
        playable_track::{PlayableTrack, ResolvedTrack},
        qobuz::token::QobuzTokenTracker,
    },
};

pub struct QobuzTrack {
    pub info: TrackInfo,
    pub album_name: Option<String>,
    pub album_url: Option<String>,
    pub artist_url: Option<String>,
    pub artist_artwork_url: Option<String>,
    pub token_tracker: Arc<QobuzTokenTracker>,
    pub client: Arc<reqwest::Client>,
}

#[async_trait]
impl PlayableTrack for QobuzTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let url = switch_media_url(&self.client, &self.token_tracker, &self.info.identifier)
            .await
            .map_err(|e| {
                format!(
                    "Qobuz: Failed to resolve media URL for {}: {e}",
                    self.info.identifier
                )
            })?
            .ok_or_else(|| "Failed to resolve Qobuz media URL".to_string())?;

        HttpTrack {
            url,
            local_addr: None,
            proxy: None,
        }
        .resolve()
        .await
    }
}

async fn switch_media_url(
    client: &Arc<reqwest::Client>,
    token_tracker: &QobuzTokenTracker,
    track_id: &str,
) -> AnyResult<Option<String>> {
    let tokens = token_tracker
        .get_tokens()
        .await
        .ok_or("Failed to get Qobuz tokens")?;

    if tokens.user_token.is_none() {
        return Ok(None);
    }

    let unix_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();

    let format_id = "5";
    let intent = "stream";

    let sig_data = format!(
        "trackgetFileUrlformat_id{format_id}intent{intent}track_id{track_id}{unix_ts}{}",
        tokens.app_secret
    );

    let mut hasher = Md5::new();
    hasher.update(sig_data.as_bytes());
    let sig = hex::encode(hasher.finalize());

    let mut url = reqwest::Url::parse("https://www.qobuz.com/api.json/0.2/track/getFileUrl")?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("request_ts", &unix_ts.to_string());
        query.append_pair("request_sig", &sig);
        query.append_pair("track_id", track_id);
        query.append_pair("format_id", format_id);
        query.append_pair("intent", intent);
    }

    let mut request = client
        .get(url)
        .header("Accept", "application/json")
        .header("x-app-id", &tokens.app_id);

    if let Some(user_token) = &tokens.user_token {
        request = request.header("x-user-auth-token", user_token);
    }

    let resp = request.send().await?;
    if !resp.status().is_success() {
        return Ok(None);
    }

    let json: serde_json::Value = resp.json().await?;
    if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
        let is_sample = json.get("sample").and_then(|v| v.as_bool()).or_else(|| {
            json.get("sample")
                .and_then(|v| v.as_str())
                .map(|s| s == "true")
        });

        if is_sample == Some(true) {
            return Ok(None);
        }
        return Ok(Some(url.to_owned()));
    }

    Ok(None)
}
