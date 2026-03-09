pub mod helpers;
pub mod metadata;
pub mod parser;
pub mod reader;
pub mod recommendations;
pub mod search;
pub mod token;
pub mod track;

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use regex::Regex;
use token::DeezerTokenTracker;
use track::DeezerTrack;

use crate::{
    protocol::tracks::LoadResult,
    sources::{SourcePlugin, plugin::PlayableTrack},
};

const PUBLIC_API_BASE: &str = "https://api.deezer.com";
const PRIVATE_API_BASE: &str = "https://www.deezer.com/ajax/gw-light.php";

pub(crate) const REC_ARTIST_PREFIX: &str = "artist=";
pub(crate) const REC_TRACK_PREFIX: &str = "track=";
pub(crate) const SHARE_URL_PREFIX: &str = "https://deezer.page.link/";

fn url_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"https?://(?:www\.)?deezer\.com/(?:[a-z]+(?:-[a-z]+)?/)?(?<type>track|album|playlist|artist)/(?<id>\d+)").unwrap()
    })
}

pub struct DeezerSource {
    client: Arc<reqwest::Client>,
    config: crate::config::DeezerConfig,
    pub token_tracker: Arc<DeezerTokenTracker>,
}

impl DeezerSource {
    pub fn new(
        config: crate::config::DeezerConfig,
        client: Arc<reqwest::Client>,
    ) -> Result<Self, String> {
        let mut arls = config.arls.clone().unwrap_or_default();
        arls.retain(|s| !s.is_empty());
        arls.sort();
        arls.dedup();

        if arls.is_empty() {
            return Err("Deezer arls must be set".to_owned());
        }
        let token_tracker = Arc::new(DeezerTokenTracker::new(client.clone(), arls));

        Ok(Self {
            client,
            config,
            token_tracker,
        })
    }
}

#[async_trait]
impl SourcePlugin for DeezerSource {
    fn name(&self) -> &str {
        "deezer"
    }

    fn can_handle(&self, identifier: &str) -> bool {
        self.search_prefixes()
            .iter()
            .any(|p| identifier.starts_with(p))
            || self
                .isrc_prefixes()
                .iter()
                .any(|p| identifier.starts_with(p))
            || self
                .rec_prefixes()
                .iter()
                .any(|p| identifier.starts_with(p))
            || identifier.starts_with(SHARE_URL_PREFIX)
            || url_regex().is_match(identifier)
    }

    fn search_prefixes(&self) -> Vec<&str> {
        vec!["dzsearch:"]
    }

    fn isrc_prefixes(&self) -> Vec<&str> {
        vec!["dzisrc:"]
    }

    fn rec_prefixes(&self) -> Vec<&str> {
        vec!["dzrec:"]
    }

    async fn load(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> LoadResult {
        for prefix in self.search_prefixes() {
            if let Some(query) = identifier.strip_prefix(prefix) {
                return self.search(query).await;
            }
        }

        for prefix in self.isrc_prefixes() {
            if let Some(isrc) = identifier.strip_prefix(prefix) {
                if let Some(track) = self.get_track_by_isrc(isrc).await {
                    return LoadResult::Track(track);
                }
                return LoadResult::Empty {};
            }
        }

        for prefix in self.rec_prefixes() {
            if let Some(query) = identifier.strip_prefix(prefix) {
                return self.get_recommendations(query).await;
            }
        }

        if identifier.starts_with(SHARE_URL_PREFIX) {
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_else(|_| (*self.client).clone());

            if let Ok(res) = client.get(identifier).send().await
                && res.status().is_redirection()
                && let Some(loc) = res.headers().get("location").and_then(|l| l.to_str().ok())
                && loc.starts_with("https://www.deezer.com/")
            {
                return self.load(loc, routeplanner).await;
            }
            return LoadResult::Empty {};
        }

        if let Some(caps) = url_regex().captures(identifier) {
            let type_ = caps.name("type").map(|m| m.as_str()).unwrap_or("");
            let id = caps.name("id").map(|m| m.as_str()).unwrap_or("");
            return match type_ {
                "track" => {
                    if let Some(json) = self.get_json_public(&format!("track/{id}")).await
                        && let Some(track) = self.parse_track(&json)
                    {
                        return LoadResult::Track(track);
                    }
                    LoadResult::Empty {}
                }
                "album" => self.get_album(id).await,
                "playlist" => self.get_playlist(id).await,
                "artist" => self.get_artist(id).await,
                _ => LoadResult::Empty {},
            };
        }

        LoadResult::Empty {}
    }

    async fn get_track(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<Box<dyn PlayableTrack>> {
        let track_id = if let Some(caps) = url_regex().captures(identifier) {
            caps.name("id").map(|m| m.as_str())?.to_owned()
        } else {
            identifier.to_owned()
        };

        Some(Box::new(DeezerTrack {
            client: self.client.clone(),
            track_id,
            arl_index: 0, // get_token will rotate
            token_tracker: self.token_tracker.clone(),
            master_key: self
                .config
                .master_decryption_key
                .clone()
                .unwrap_or_default(),
            local_addr: routeplanner.and_then(|rp| rp.get_address()),
            proxy: self.config.proxy.clone(),
        }))
    }

    async fn load_search(
        &self,
        query: &str,
        types: &[String],
        _routeplanner: Option<Arc<dyn crate::routeplanner::RoutePlanner>>,
    ) -> Option<crate::protocol::tracks::SearchResult> {
        let mut q = query;
        for prefix in self.search_prefixes() {
            if let Some(stripped) = query.strip_prefix(prefix) {
                q = stripped;
                break;
            }
        }
        self.get_autocomplete(q, types).await
    }
}
