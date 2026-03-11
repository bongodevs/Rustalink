use std::sync::Arc;

use dashmap::DashMap;
use perf_monitor::cpu::ProcessStat;
use sysinfo::System;

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
    pub system_state: parking_lot::Mutex<System>,
    pub last_system_refresh: parking_lot::Mutex<std::time::Instant>,
    pub process_stat: parking_lot::Mutex<ProcessStat>,
}
