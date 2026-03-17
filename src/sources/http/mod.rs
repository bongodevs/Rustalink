pub mod reader;
pub mod track;

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use regex::Regex;
use symphonia::core::{
    codecs::CODEC_TYPE_NULL,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTagKey},
    probe::Hint,
};
use tracing::{debug, warn};
pub use track::HttpTrack;

use crate::{
    common::types::AnyResult,
    protocol::tracks::{LoadResult, Track, TrackInfo},
    sources::{SourcePlugin, playable_track::PlayableTrack},
};

fn url_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^(?:https?|icy)://").unwrap())
}

pub struct HttpSource;

impl Default for HttpSource {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpSource {
    pub fn new() -> Self {
        Self
    }

    async fn probe_metadata(
        url: String,
        local_addr: Option<std::net::IpAddr>,
    ) -> AnyResult<TrackInfo> {
        let source = reader::HttpReader::new(&url, local_addr, None).await?;
        let mut hint = Hint::new();

        if let Some(content_type) = source.content_type() {
            hint.mime_type(content_type.as_str());
        }

        let mss = MediaSourceStream::new(Box::new(source), Default::default());

        if let Some(ext) = std::path::Path::new(&url)
            .extension()
            .and_then(|s| s.to_str())
        {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe().format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )?;

        let mut format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("no audio track found")?;

        let duration = if let Some(n_frames) = track.codec_params.n_frames {
            if let Some(rate) = track.codec_params.sample_rate {
                (n_frames as f64 / rate as f64 * 1000.0) as u64
            } else {
                0
            }
        } else {
            0
        };

        let mut title = String::new();
        let mut author = String::new();

        if let Some(metadata) = format.metadata().current() {
            if let Some(tag) = metadata
                .tags()
                .iter()
                .find(|t| t.std_key == Some(StandardTagKey::TrackTitle))
            {
                title = tag.value.to_string();
            }
            if let Some(tag) = metadata
                .tags()
                .iter()
                .find(|t| t.std_key == Some(StandardTagKey::Artist))
            {
                author = tag.value.to_string();
            }
        }

        if title.is_empty() {
            title = url
                .split('/')
                .next_back()
                .and_then(|s| s.split('?').next())
                .unwrap_or("Unknown Title")
                .to_owned();
        }

        if author.is_empty() {
            author = "Unknown Artist".to_owned();
        }

        Ok(TrackInfo {
            identifier: url.clone(),
            author,
            length: duration,
            is_seekable: true,
            is_stream: false,
            position: 0,
            title,
            uri: Some(url),
            source_name: "http".to_owned(),
            artwork_url: None,
            isrc: None,
        })
    }
}

#[async_trait]
impl SourcePlugin for HttpSource {
    fn name(&self) -> &str {
        "http"
    }

    fn can_handle(&self, identifier: &str) -> bool {
        url_regex().is_match(identifier)
    }

    async fn load(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> LoadResult {
        debug!("Probing HTTP source: {identifier}");

        let identifier = identifier.to_owned();
        let local_addr = routeplanner.as_ref().and_then(|rp| rp.get_address());

        match HttpSource::probe_metadata(identifier, local_addr).await {
            Ok(info) => LoadResult::Track(Track::new(info)),
            Err(e) => {
                warn!("Probing failed: {e}");
                LoadResult::Empty {}
            }
        }
    }

    async fn get_track(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<Arc<dyn PlayableTrack>> {
        let clean = identifier
            .trim()
            .trim_start_matches('<')
            .trim_end_matches('>');

        if self.can_handle(clean) {
            Some(Arc::new(HttpTrack {
                url: clean.to_owned(),
                local_addr: routeplanner.and_then(|rp| rp.get_address()),
                proxy: None,
            }))
        } else {
            None
        }
    }
}
