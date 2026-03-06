use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

use dashmap::DashMap;

use crate::{
    common::types::SessionId,
    routeplanner::RoutePlanner,
    server::session::Session,
    sources::{SourceManager, youtube::YoutubeStreamContext},
};

pub type SessionMap = DashMap<SessionId, Arc<Session>>;

pub struct AppState {
    pub start_time: std::time::Instant,
    pub sessions: SessionMap,
    pub resumable_sessions: SessionMap,
    pub routeplanner: Option<Arc<dyn RoutePlanner>>,
    pub source_manager: Arc<SourceManager>,
    pub lyrics_manager: Arc<crate::lyrics::LyricsManager>,
    pub config: crate::config::AppConfig,
    pub youtube: Option<Arc<YoutubeStreamContext>>,
    pub total_players: AtomicI32,
    pub playing_players: AtomicI32,
}

impl AppState {
    pub fn player_created(&self) {
        self.total_players.fetch_add(1, Ordering::Relaxed);
    }

    pub fn player_destroyed(&self, was_playing: bool) {
        self.total_players.fetch_sub(1, Ordering::Relaxed);
        if was_playing {
            self.playing_players.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn playback_started(&self) {
        self.playing_players.fetch_add(1, Ordering::Relaxed);
    }

    pub fn playback_stopped(&self) {
        self.playing_players.fetch_sub(1, Ordering::Relaxed);
    }
}
