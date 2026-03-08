use serde::{Deserialize, Serialize};

use super::HttpProxyConfig;
use crate::config::sources::{default_false, default_limit_10};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VkMusicConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub user_token: Option<String>,
    pub user_cookie: Option<String>,
    #[serde(default = "default_limit_10")]
    pub search_limit: usize,
    #[serde(default = "default_one")]
    pub playlist_load_limit: usize,
    #[serde(default = "default_one")]
    pub artist_load_limit: usize,
    #[serde(default = "default_limit_10")]
    pub recommendations_load_limit: usize,
    pub proxy: Option<HttpProxyConfig>,
}

impl Default for VkMusicConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            user_token: None,
            user_cookie: None,
            search_limit: 10,
            playlist_load_limit: 1,
            artist_load_limit: 1,
            recommendations_load_limit: 10,
            proxy: None,
        }
    }
}

fn default_one() -> usize {
    1
}
