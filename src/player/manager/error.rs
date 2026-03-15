use super::super::context::PlayerContext;
use crate::{
    protocol::{
        self,
        events::{RustalinkEvent, TrackEndReason, TrackException},
    },
    server::Session,
};

/// Emit `TrackException` followed by `TrackEnd: LoadFailed`.
pub async fn send_load_failed(player: &PlayerContext, session: &Session, message: String) {
    let Some(track) = player.to_player_response().await.track else {
        return;
    };
    let guild_id = player.guild_id.clone();

    let friendly_message =
        if message.contains("This video ") || message.contains("All clients failed to resolve") {
            message.clone()
        } else {
            "Something broke when playing the track.".to_string()
        };

    let short_cause = crate::common::utils::shorten_error_cause(&message);

    session.send_message(&protocol::OutgoingMessage::Event {
        event: Box::new(RustalinkEvent::TrackException {
            guild_id: guild_id.clone(),
            track: track.clone(),
            exception: TrackException {
                message: Some(friendly_message),
                severity: crate::common::Severity::Common,
                cause: short_cause,
                cause_stack_trace: Some(message),
            },
        }),
    });

    session.send_message(&protocol::OutgoingMessage::Event {
        event: Box::new(RustalinkEvent::TrackEnd {
            guild_id,
            track,
            reason: TrackEndReason::LoadFailed,
        }),
    });
}
