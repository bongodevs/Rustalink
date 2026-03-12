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

use crate::protocol::tracks::TrackInfo;

pub struct AmazonTrackData {
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub duration_ms: u64,
    pub artwork_url: Option<String>,
    pub isrc: Option<String>,
}

impl AmazonTrackData {
    pub fn into_track_info(self) -> TrackInfo {
        let uri = format!("https://music.amazon.com/tracks/{}", self.track_id);
        TrackInfo {
            identifier: self.track_id,
            is_seekable: true,
            author: self.artist,
            length: self.duration_ms,
            is_stream: false,
            position: 0,
            title: self.title,
            uri: Some(uri),
            artwork_url: self.artwork_url,
            isrc: self.isrc,
            source_name: "amazonmusic".to_string(),
        }
    }
}
