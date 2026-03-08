use std::{net::IpAddr, sync::Arc};

use flume::{Receiver, Sender};

use crate::{
    audio::processor::DecoderCommand,
    sources::{http::HttpTrack, plugin::PlayableTrack},
};

pub struct VkMusicTrack {
    pub stream_url: String,
    pub local_addr: Option<IpAddr>,
    pub proxy: Option<crate::config::HttpProxyConfig>,
}

impl PlayableTrack for VkMusicTrack {
    fn start_decoding(
        &self,
        config: crate::config::player::PlayerConfig,
    ) -> (
        Receiver<crate::audio::buffer::PooledBuffer>,
        Sender<DecoderCommand>,
        Receiver<String>,
        Option<Receiver<Arc<Vec<u8>>>>,
    ) {
        HttpTrack {
            url: self.stream_url.clone(),
            local_addr: self.local_addr,
            proxy: self.proxy.clone(),
        }
        .start_decoding(config)
    }
}
