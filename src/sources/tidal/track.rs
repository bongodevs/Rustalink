// Copyright (c) 2026 appujet, notdeltaxd and contributors
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use std::sync::Arc;

use async_trait::async_trait;
use tracing::debug;

use super::client::TidalClient;
use crate::{
    audio::source::HttpSource,
    common::types::AudioFormat,
    sources::playable_track::{PlayableTrack, ResolvedTrack},
};

pub struct TidalTrack {
    pub identifier: String,
    pub stream_url: String,
    pub kind: AudioFormat,
    pub client: Arc<TidalClient>,
}

#[async_trait]
impl PlayableTrack for TidalTrack {
    async fn resolve(&self) -> Result<ResolvedTrack, String> {
        debug!(
            "TidalTrack: resolving {} with quality {}",
            self.identifier, self.client.quality
        );

        let client_inner = (*self.client.inner).clone();
        let stream_url = self.stream_url.clone();
        let kind = self.kind;

        let reader = HttpSource::new(client_inner, &stream_url)
            .await
            .map(|r| Box::new(r) as Box<dyn symphonia::core::io::MediaSource>)
            .map_err(|e| format!("Failed to initialize source: {e}"))?;

        Ok(ResolvedTrack::new(reader, Some(kind)))
    }
}
