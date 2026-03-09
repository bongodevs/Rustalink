use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::{
    config::AppConfig,
    protocol::{models::LyricsData, tracks::TrackInfo},
};

pub mod deezer;
pub mod genius;
pub mod letrasmus;
pub mod lrclib;
pub mod musixmatch;
pub mod netease;
pub mod utils;
pub mod yandex;
pub mod youtubemusic;

use deezer::DeezerProvider;
use genius::GeniusProvider;
use letrasmus::LetrasMusProvider;
use lrclib::LrcLibProvider;
use musixmatch::MusixmatchProvider;
use netease::NeteaseProvider;
use yandex::YandexProvider;
use youtubemusic::YoutubeMusicLyricsProvider;

#[async_trait]
pub trait LyricsProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn load_lyrics(&self, track: &TrackInfo) -> Option<LyricsData>;
}

pub struct LyricsManager {
    pub providers: Vec<Arc<dyn LyricsProvider>>,
}

impl LyricsManager {
    pub fn new(config: &AppConfig) -> Self {
        let mut providers: Vec<Arc<dyn LyricsProvider>> = Vec::new();

        macro_rules! register_provider {
            ($enabled:expr, $name:literal, $ctor:expr) => {
                if $enabled {
                    providers.push(Arc::new($ctor));
                    tracing::info!("Loaded lyrics provider: {}", $name);
                }
            };
        }

        register_provider!(
            config.lyrics.youtubemusic,
            "YoutubeMusic",
            YoutubeMusicLyricsProvider::new()
        );
        register_provider!(config.lyrics.lrclib, "LRCLib", LrcLibProvider::new());
        register_provider!(config.lyrics.genius, "Genius", GeniusProvider::new());

        let deezer_proxy = config
            .sources
            .deezer
            .as_ref()
            .and_then(|d| d.proxy.as_ref());
        register_provider!(
            config.lyrics.deezer,
            "Deezer",
            DeezerProvider::new(deezer_proxy)
        );

        register_provider!(
            config.lyrics.musixmatch,
            "Musixmatch",
            MusixmatchProvider::new()
        );
        register_provider!(
            config.lyrics.letrasmus,
            "Letras.mus",
            LetrasMusProvider::new()
        );
        register_provider!(config.lyrics.netease, "NetEase", NeteaseProvider::new());

        if let Some(yandex_cfg) = config.sources.yandexmusic.as_ref() {
            let yandex_token = yandex_cfg.access_token.as_ref();
            let yandex_token_provided = yandex_token.is_some_and(|t| !t.is_empty());

            if config.lyrics.yandex && !yandex_token_provided {
                tracing::warn!(
                    "Yandex lyrics enabled but access_token is missing; it will be disabled."
                );
            }

            let yandex_lyrics_cfg = crate::config::lyrics::YandexLyricsConfig {
                access_token: yandex_cfg.access_token.clone(),
            };
            let yandex_proxy = yandex_cfg.proxy.as_ref();
            register_provider!(
                config.lyrics.yandex && yandex_token_provided,
                "Yandex Music",
                YandexProvider::new(&yandex_lyrics_cfg, yandex_proxy)
            );
        }

        Self { providers }
    }

    pub async fn load_lyrics(&self, track: &TrackInfo) -> Option<LyricsData> {
        self.load_lyrics_ext(track, false).await
    }

    pub async fn load_lyrics_ext(
        &self,
        track: &TrackInfo,
        skip_track_source: bool,
    ) -> Option<LyricsData> {
        let mut futures = FuturesUnordered::new();

        for provider in &self.providers {
            if skip_track_source && provider.name().eq_ignore_ascii_case(&track.source_name) {
                continue;
            }
            let provider = provider.clone();
            let track = track.clone();
            futures.push(async move { provider.load_lyrics(&track).await });
        }

        let mut fallback_text: Option<LyricsData> = None;

        while let Some(result) = futures.next().await {
            if let Some(lyrics) = result {
                if lyrics.lines.is_some() {
                    return Some(lyrics);
                } else {
                    fallback_text.get_or_insert(lyrics);
                }
            }
        }

        fallback_text
    }
}
