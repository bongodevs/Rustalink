use std::sync::{
    Arc,
    atomic::{AtomicI64, AtomicU64},
};

use tracing::error;

use crate::{
    audio::filters::FilterChain,
    common::types::{ChannelId, GuildId, Shared, UserId},
    gateway::{VoiceEngine, VoiceGateway},
    player::VoiceConnectionState,
    protocol::RustalinkEvent,
};

pub struct VoiceConnectConfig {
    pub engine: Shared<VoiceEngine>,
    pub guild_id: GuildId,
    pub user_id: UserId,
    pub voice: VoiceConnectionState,
    pub filter_chain: Shared<FilterChain>,
    pub ping: Arc<AtomicI64>,
    pub event_tx: Option<tokio::sync::mpsc::UnboundedSender<RustalinkEvent>>,
    pub frames_sent: Arc<AtomicU64>,
    pub frames_nulled: Arc<AtomicU64>,
}

/// Spawns the voice gateway task for the given guild.
pub async fn connect_voice(config: VoiceConnectConfig) -> tokio::task::JoinHandle<()> {
    let Some(channel_id) = config
        .voice
        .channel_id
        .as_deref()
        .and_then(|id| id.parse::<u64>().ok())
        .map(ChannelId)
    else {
        error!("Failed to connect voice: channel_id is missing or invalid");
        return tokio::spawn(async {});
    };

    let mixer = config.engine.lock().await.mixer.clone();

    let gateway = VoiceGateway::new(crate::gateway::session::VoiceGatewayConfig {
        guild_id: config.guild_id,
        user_id: config.user_id,
        channel_id,
        session_id: config.voice.session_id.into(),
        token: config.voice.token,
        endpoint: config.voice.endpoint,
        mixer,
        filter_chain: config.filter_chain,
        ping: config.ping,
        event_tx: config.event_tx,
        frames_sent: config.frames_sent,
        frames_nulled: config.frames_nulled,
    });

    {
        let mut engine = config.engine.lock().await;
        engine.dave = Some(gateway.dave.clone());
    }

    tokio::spawn(async move {
        if let Err(e) = gateway.run().await {
            error!("Voice gateway error: {}", e);
        }
    })
}
