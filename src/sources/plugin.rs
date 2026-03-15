use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    config::HttpProxyConfig,
    protocol::tracks::{LoadResult, SearchResult},
    routeplanner::RoutePlanner,
    sources::playable_track::BoxedTrack,
};

#[async_trait]
pub trait SourcePlugin: Send + Sync {
    fn name(&self) -> &str;

    fn can_handle(&self, identifier: &str) -> bool;

    async fn load(
        &self,
        identifier: &str,
        routeplanner: Option<Arc<dyn RoutePlanner>>,
    ) -> LoadResult;

    async fn get_track(
        &self,
        _identifier: &str,
        _routeplanner: Option<Arc<dyn RoutePlanner>>,
    ) -> Option<BoxedTrack> {
        None
    }

    async fn load_search(
        &self,
        _query: &str,
        _types: &[String],
        _routeplanner: Option<Arc<dyn RoutePlanner>>,
    ) -> Option<SearchResult> {
        None
    }

    fn get_proxy_config(&self) -> Option<HttpProxyConfig> {
        None
    }

    fn search_prefixes(&self) -> Vec<&str> {
        vec![]
    }

    fn isrc_prefixes(&self) -> Vec<&str> {
        vec![]
    }

    fn rec_prefixes(&self) -> Vec<&str> {
        vec![]
    }

    fn is_mirror(&self) -> bool {
        false
    }
}
