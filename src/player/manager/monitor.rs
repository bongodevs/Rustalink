use std::sync::{Arc, atomic::Ordering};

use tracing::warn;

use super::lyrics::sync_lyrics;
use crate::{
    audio::playback::{PlaybackState, StuckDetector, TrackHandle},
    common::{types::GuildId, utils::now_ms},
    player::state::PlayerState,
    protocol::{
        self,
        events::{RustalinkEvent, TrackEndReason, TrackException},
        models::LyricsData,
        tracks::Track,
    },
    server::Session,
};

pub struct MonitorCtx {
    pub guild_id: GuildId,
    pub handle: TrackHandle,
    pub err_rx: flume::Receiver<String>,
    pub session: Arc<Session>,
    pub track: Track,
    pub stop_signal: Arc<std::sync::atomic::AtomicBool>,
    pub ping: Arc<std::sync::atomic::AtomicI64>,
    pub update_every_n: u64,
    pub lyrics_subscribed: Arc<std::sync::atomic::AtomicBool>,
    pub lyrics_data: Arc<tokio::sync::Mutex<Option<LyricsData>>>,
    pub last_lyric_index: Arc<std::sync::atomic::AtomicI64>,
    pub end_time_ms: Option<u64>,
    pub stuck_detector: Arc<StuckDetector>,
}

pub async fn monitor_loop(ctx: MonitorCtx) {
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    let mut tick: u64 = 0;
    let mut last_pos = ctx.handle.get_position();

    send_player_update(&ctx, last_pos);

    loop {
        interval.tick().await;
        tick = tick.wrapping_add(1);

        if ctx.stop_signal.load(Ordering::Acquire) {
            break;
        }

        let state = ctx.handle.get_state();

        if state == PlaybackState::Stopped {
            handle_playback_stopped(&ctx).await;
            break;
        }

        let cur_pos = ctx.handle.get_position();

        if let Some(end_ms) = ctx.end_time_ms
            && cur_pos >= end_ms
            && state == PlaybackState::Playing
        {
            handle_track_end_marker(&ctx).await;
            break;
        }

        if state == PlaybackState::Playing {
            if cur_pos != last_pos {
                ctx.stuck_detector.reset_stuck_flag();
            }

            if ctx.stuck_detector.check_stuck() {
                send_stuck_event(&ctx, cur_pos);
            }
        }

        last_pos = cur_pos;

        if tick.is_multiple_of(ctx.update_every_n) {
            send_player_update(&ctx, cur_pos);
        }

        if ctx.lyrics_subscribed.load(Ordering::Relaxed) {
            sync_lyrics(
                &ctx.guild_id,
                cur_pos,
                &ctx.last_lyric_index,
                &ctx.lyrics_data,
                &ctx.session,
            )
            .await;
        }
    }
}

async fn handle_playback_stopped(ctx: &MonitorCtx) {
    if ctx.stop_signal.load(Ordering::Acquire) {
        return;
    }

    let reason = match ctx.err_rx.try_recv() {
        Ok(err) => {
            warn!("[{}] mid-playback decoder error: {}", ctx.guild_id, err);

            let message = if err.contains("This video ") || err.contains("This is a private video")
            {
                err.clone()
            } else {
                "Something went wrong when decoding the track.".to_owned()
            };

            let short_cause = crate::common::utils::shorten_error_cause(&err);

            ctx.session.send_message(&protocol::OutgoingMessage::Event {
                event: Box::new(RustalinkEvent::TrackException {
                    guild_id: ctx.guild_id.clone(),
                    track: ctx.track.clone(),
                    exception: TrackException {
                        message: Some(message),
                        severity: crate::common::Severity::Fault,
                        cause: short_cause,
                        cause_stack_trace: Some(err),
                    },
                }),
            });
            TrackEndReason::LoadFailed
        }
        Err(_) => TrackEndReason::Finished,
    };

    clear_player_state(ctx).await;

    ctx.session.send_message(&protocol::OutgoingMessage::Event {
        event: Box::new(RustalinkEvent::TrackEnd {
            guild_id: ctx.guild_id.clone(),
            track: ctx.track.clone(),
            reason,
        }),
    });
}

async fn handle_track_end_marker(ctx: &MonitorCtx) {
    ctx.stop_signal.store(true, Ordering::Release);
    ctx.handle.stop();

    clear_player_state(ctx).await;

    ctx.session.send_message(&protocol::OutgoingMessage::Event {
        event: Box::new(RustalinkEvent::TrackEnd {
            guild_id: ctx.guild_id.clone(),
            track: ctx.track.clone(),
            reason: TrackEndReason::Finished,
        }),
    });
}

fn send_stuck_event(ctx: &MonitorCtx, cur_pos: u64) {
    let threshold = ctx.stuck_detector.threshold_ms();

    ctx.session.send_message(&protocol::OutgoingMessage::Event {
        event: Box::new(RustalinkEvent::TrackStuck {
            guild_id: ctx.guild_id.clone(),
            track: ctx.track.clone(),
            threshold_ms: threshold,
        }),
    });

    send_player_update(ctx, cur_pos);

    warn!(
        "[{}] Track stuck: no frames received for >= {}ms",
        ctx.guild_id, threshold
    );
}

fn send_player_update(ctx: &MonitorCtx, cur_pos: u64) {
    ctx.session
        .send_message(&protocol::OutgoingMessage::PlayerUpdate {
            guild_id: ctx.guild_id.clone(),
            state: PlayerState {
                time: now_ms(),
                position: cur_pos,
                connected: true,
                ping: ctx.ping.load(Ordering::Acquire),
            },
        });
}

async fn clear_player_state(ctx: &MonitorCtx) {
    if let Some(player_arc) = ctx
        .session
        .players
        .get(&ctx.guild_id)
        .map(|kv| kv.value().clone())
    {
        let mut p = player_arc.write().await;
        if p.track_handle
            .as_ref()
            .map(|h| h.is_same(&ctx.handle))
            .unwrap_or(false)
        {
            p.track = None;
            p.track_info = None;
            p.track_handle = None;
        }
    }
}
