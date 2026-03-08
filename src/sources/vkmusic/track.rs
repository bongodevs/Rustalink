use std::net::IpAddr;

use crate::sources::{
    http::HttpTrack,
    plugin::{DecoderOutput, PlayableTrack},
};

pub struct VkMusicTrack {
    pub stream_url: String,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<crate::config::HttpProxyConfig>,
}

impl PlayableTrack for VkMusicTrack {
    fn start_decoding(&self, config: crate::config::player::PlayerConfig) -> DecoderOutput {
        HttpTrack {
            url: self.stream_url.clone(),
            local_addr: self.local_addr,
            proxy: self.proxy.clone(),
        }
        .start_decoding(config)
    }
}
