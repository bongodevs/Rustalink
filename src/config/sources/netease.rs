use serde::{Deserialize, Serialize};

use super::HttpProxyConfig;
use crate::config::sources::{default_false, default_limit_10};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NeteaseMusicConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_limit_10")]
    pub search_limit: usize,
    pub proxy: Option<HttpProxyConfig>,
}

impl Default for NeteaseMusicConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            search_limit: 10,
            proxy: None,
        }
    }
}
