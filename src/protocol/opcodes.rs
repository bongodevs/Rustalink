use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::{
    player::VoiceConnectionState,
    server::{AppState, Session},
};

#[derive(Deserialize, Debug)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum IncomingMessage {
    VoiceUpdate {
        guild_id: crate::common::types::GuildId,
        session_id: String,
        channel_id: Option<String>,
        event: Value,
    },
    Play {
        guild_id: crate::common::types::GuildId,
        track: String,
    },
    Stop {
        guild_id: crate::common::types::GuildId,
    },
    Destroy {
        guild_id: crate::common::types::GuildId,
    },
}

pub async fn handle_op(
    op: IncomingMessage,
    state: &Arc<AppState>,
    session_id: &crate::common::types::SessionId,
) -> Result<(), String> {
    let session: Arc<Session> = state
        .sessions
        .get(session_id)
        .map(|s| s.clone())
        .ok_or_else(|| "Session not found".to_string())?;

    match op {
        IncomingMessage::VoiceUpdate {
            guild_id,
            session_id: voice_session_id,
            channel_id,
            event,
        } => {
            handle_voice_update(
                &session,
                state,
                guild_id,
                voice_session_id,
                channel_id,
                event,
            )
            .await
        }
        IncomingMessage::Play { guild_id, track } => {
            handle_play(&session, state, guild_id, track).await
        }
        IncomingMessage::Stop { guild_id } => {
            if let Some(player_arc) = session.players.get(&guild_id).map(|kv| kv.value().clone()) {
                let mut player = player_arc.write().await;
                player.stop_track();
            }
            Ok(())
        }
        IncomingMessage::Destroy { guild_id } => {
            session.destroy_player(&guild_id).await;
            Ok(())
        }
    }
}

async fn handle_voice_update(
    session: &Arc<Session>,
    state: &Arc<AppState>,
    guild_id: crate::common::types::GuildId,
    voice_session_id: String,
    channel_id: Option<String>,
    event: Value,
) -> Result<(), String> {
    let token = event
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or("Missing token in voice update event")?
        .to_string();
    let endpoint = event
        .get("endpoint")
        .and_then(|v| v.as_str())
        .ok_or("Missing endpoint in voice update event")?
        .to_string();

    let player_arc = session.get_or_create_player(guild_id.clone(), state.clone());

    let Some(uid) = session.user_id else {
        return Ok(());
    };

    let mut changed = false;
    {
        let mut player = player_arc.write().await;
        if player.voice.token != token
            || player.voice.endpoint != endpoint
            || player.voice.session_id != voice_session_id
            || player.voice.channel_id != channel_id
        {
            player.voice = VoiceConnectionState {
                token,
                endpoint,
                session_id: voice_session_id,
                channel_id,
            };
            changed = true;
        }
    }

    let needs_task = {
        let player = player_arc.read().await;
        player.gateway_task.is_none()
    };

    if changed || needs_task {
        let mut player = player_arc.write().await;
        let engine = player.engine.clone();
        let guild = player.guild_id.clone();
        let voice_state = player.voice.clone();
        let filter_chain = player.filter_chain.clone();
        let ping = player.ping.clone();

        if let Some(task) = player.gateway_task.take() {
            task.abort();
        }

        let frames_sent = player.frames_sent.clone();
        let frames_nulled = player.frames_nulled.clone();

        drop(player);

        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
        let session_clone = session.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let msg = crate::protocol::OutgoingMessage::Event {
                    event: Box::new(event),
                };
                session_clone.send_message(&msg);
            }
        });

        let new_task = crate::server::connect_voice(crate::server::voice::VoiceConnectConfig {
            engine,
            guild_id: guild,
            user_id: uid,
            voice: voice_state,
            filter_chain,
            ping,
            event_tx: Some(event_tx),
            frames_sent,
            frames_nulled,
        })
        .await;

        let mut player_w = player_arc.write().await;
        session.register_task(new_task.abort_handle());
        player_w.gateway_task = Some(new_task);
    }

    Ok(())
}

async fn handle_play(
    session: &Arc<Session>,
    state: &Arc<AppState>,
    guild_id: crate::common::types::GuildId,
    track: String,
) -> Result<(), String> {
    let player_arc = session.get_or_create_player(guild_id, state.clone());
    let mut player = player_arc.write().await;

    crate::player::start_playback(
        &mut player,
        crate::player::manager::start::PlaybackStartConfig {
            track,
            session: session.clone(),
            source_manager: state.source_manager.clone(),
            lyrics_manager: state.lyrics_manager.clone(),
            routeplanner: state.routeplanner.clone(),
            update_interval_secs: state.config.server.player_update_interval,
            user_data: None,
            end_time: None,
            start_time_ms: None,
        },
    )
    .await;

    Ok(())
}
