use std::{net::IpAddr, sync::Arc};

use async_trait::async_trait;
use regex::Regex;
use tracing::debug;

use super::utils;
use crate::{
    config::HttpProxyConfig,
    sources::{
        http::HttpTrack,
        playable_track::{PlayableTrack, ResolvedTrack},
    },
};

pub struct YandexMusicTrack {
    pub client: Arc<reqwest::Client>,
    pub track_id: String,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<HttpProxyConfig>,
}

#[async_trait]
impl PlayableTrack for YandexMusicTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        let stream_url = fetch_download_url(&self.client, &self.track_id)
            .await
            .ok_or_else(|| {
                format!(
                    "Failed to fetch Yandex Music stream URL for track ID {}",
                    self.track_id
                )
            })?;

        debug!("Yandex Music stream URL: {}", stream_url);

        let http_track = HttpTrack {
            url: stream_url,
            local_addr: self.local_addr,
            proxy: self.proxy.clone(),
        };

        http_track.resolve().await
    }

}


pub(super) async fn fetch_download_url(
    client: &Arc<reqwest::Client>,
    id: &str,
) -> Option<String> {
    let url  = format!("https://api.music.yandex.net/tracks/{}/download-info", id);
    let resp = client.get(url).send().await.ok()?;
    let data: serde_json::Value = resp.json().await.ok()?;

    let results = data["result"].as_array()?;

    let mut mp3_items: Vec<_> = results
        .iter()
        .filter(|item| item["codec"].as_str() == Some("mp3"))
        .collect();

    mp3_items.sort_by_key(|item| item["bitrateInKbps"].as_u64().unwrap_or(0));
    let best_mp3          = mp3_items.last()?;
    let download_info_url = best_mp3["downloadInfoUrl"].as_str()?;

    let xml_resp = client.get(download_info_url).send().await.ok()?;
    let xml_text = xml_resp.text().await.ok()?;

    let get_tag = |text: &str, tag: &str| -> Option<String> {
        let pattern = format!("<{tag}>(?P<val>[^<]+)</{tag}>");
        let re      = Regex::new(&pattern).ok()?;
        re.captures(text)?.name("val")?.as_str().to_string().into()
    };

    let host = get_tag(&xml_text, "host")?;
    let path = get_tag(&xml_text, "path")?;
    let ts   = get_tag(&xml_text, "ts")?;
    let s    = get_tag(&xml_text, "s")?;

    let md5 = utils::generate_download_sign(&path, &s);

    Some(format!("https://{}/get-mp3/{}/{}{}", host, md5, ts, path))
}