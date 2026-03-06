use std::{
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::error;

use super::types::map_boxed_err;
use crate::{
    audio::{Mixer, engine::Encoder, filters::FilterChain},
    common::types::{AnyResult, Shared},
    gateway::{
        DaveHandler, UdpBackend,
        constants::{
            DISCOVERY_PACKET_SIZE, FRAME_DURATION_MS, IP_DISCOVERY_TIMEOUT_SECS,
            MAX_OPUS_FRAME_SIZE, MAX_SILENCE_FRAMES, PCM_FRAME_SAMPLES,
        },
    },
};

const PCM_FRAME_SIZE: usize = PCM_FRAME_SAMPLES * 2;
const TRAILING_SILENCE_FRAMES: u32 = 5;

pub async fn discover_ip(
    socket: &tokio::net::UdpSocket,
    addr: SocketAddr,
    ssrc: u32,
) -> AnyResult<(String, u16)> {
    let mut packet = [0u8; DISCOVERY_PACKET_SIZE];
    packet[0..2].copy_from_slice(&1u16.to_be_bytes());
    packet[2..4].copy_from_slice(&70u16.to_be_bytes());
    packet[4..8].copy_from_slice(&ssrc.to_be_bytes());

    socket.send_to(&packet, addr).await.map_err(map_boxed_err)?;

    let mut buf = [0u8; DISCOVERY_PACKET_SIZE];
    match tokio::time::timeout(
        Duration::from_secs(IP_DISCOVERY_TIMEOUT_SECS),
        socket.recv(&mut buf),
    )
    .await
    {
        Ok(Ok(n)) if n >= DISCOVERY_PACKET_SIZE => {
            let ip = std::str::from_utf8(&buf[8..72])
                .map_err(map_boxed_err)?
                .trim_matches('\0')
                .to_string();
            let port = u16::from_be_bytes([buf[72], buf[73]]);
            Ok((ip, port))
        }
        Ok(Ok(_)) => Err(map_boxed_err("Malformed IP discovery response")),
        Ok(Err(e)) => Err(map_boxed_err(e)),
        Err(_) => Err(map_boxed_err("IP discovery timed out")),
    }
}

pub struct SpeakConfig {
    pub mixer: Shared<Mixer>,
    pub socket: Arc<tokio::net::UdpSocket>,
    pub addr: SocketAddr,
    pub ssrc: u32,
    pub key: [u8; 32],
    pub mode: String,
    pub dave: Shared<DaveHandler>,
    pub filter_chain: Shared<FilterChain>,
    pub frames_sent: Arc<std::sync::atomic::AtomicU64>,
    pub frames_nulled: Arc<std::sync::atomic::AtomicU64>,
    pub cancel_token: CancellationToken,
    pub speaking_tx: UnboundedSender<bool>,
}

pub async fn speak_loop(config: SpeakConfig) -> AnyResult<()> {
    let mut encoder = Encoder::new().map_err(map_boxed_err)?;
    let mut udp = UdpBackend::new(
        config.socket,
        config.addr,
        config.ssrc,
        config.key,
        &config.mode,
    )?;

    let frame_period = Duration::from_millis(FRAME_DURATION_MS);
    let mut deadline = Instant::now() + frame_period;

    let mut pcm = vec![0i16; PCM_FRAME_SIZE];
    let mut opus = vec![0u8; MAX_OPUS_FRAME_SIZE];
    let mut ts_pcm = vec![0i16; PCM_FRAME_SIZE];

    let mut silence_cnt: u32 = 0;
    let mut trailing_cnt: u32 = 0;
    let mut is_speaking = false;

    loop {
        tokio::select! {
            biased;
            _ = config.cancel_token.cancelled() => break,
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {}
        }
        deadline += frame_period;

        let (opus_raw, has_audio, do_encode) = {
            let mut m = config.mixer.lock().await;
            if let Some(frame) = m.take_opus_frame() {
                (Some(frame), true, false)
            } else {
                let mixed = m.mix(&mut pcm);
                (None, mixed, true)
            }
        };

        let active = has_audio || (trailing_cnt > 0);
        if active != is_speaking {
            is_speaking = active;
            let _ = config.speaking_tx.send(is_speaking);
        }

        if let Some(frame) = opus_raw {
            silence_cnt = 0;
            trailing_cnt = TRAILING_SILENCE_FRAMES;
            config.frames_sent.fetch_add(1, Ordering::Relaxed);
            if let Ok(enc) = config.dave.lock().await.encrypt_opus(&frame) {
                let _ = udp.send_opus_packet(&enc).await;
            }
            continue;
        }

        if !do_encode {
            continue;
        }

        if has_audio {
            silence_cnt = 0;
            trailing_cnt = TRAILING_SILENCE_FRAMES;
            config.frames_sent.fetch_add(1, Ordering::Relaxed);
        } else {
            silence_cnt += 1;
            config.frames_nulled.fetch_add(1, Ordering::Relaxed);

            if trailing_cnt > 0 {
                trailing_cnt -= 1;
                pcm.fill(0);
            } else if silence_cnt > MAX_SILENCE_FRAMES {
                continue;
            } else {
                pcm.fill(0);
            }
        }

        let mut ready = true;
        let mut use_ts = false;
        {
            let mut fc = config.filter_chain.lock().await;
            if fc.is_active() {
                fc.process(&mut pcm);
                if fc.has_timescale() {
                    ready = fc.fill_frame(&mut ts_pcm);
                    use_ts = true;
                }
            }
        }

        if !ready {
            continue;
        }

        let pcm_ref = if use_ts { &ts_pcm } else { &pcm };

        let size = match encoder.encode(pcm_ref, &mut opus) {
            Ok(s) => s,
            Err(e) => {
                error!("Opus error: {e}");
                continue;
            }
        };

        if size > 0
            && let Ok(enc) = config.dave.lock().await.encrypt_opus(&opus[..size])
        {
            let _ = udp.send_opus_packet(&enc).await;
        }
    }

    Ok(())
}
