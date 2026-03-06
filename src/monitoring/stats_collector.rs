use std::sync::atomic::{AtomicU64, Ordering};

use crate::{protocol, server::AppState};

/// Entry point for gathering all operational metrics.
pub fn collect_stats(
    app_state: &AppState,
    current_session: Option<&crate::server::session::Session>,
) -> protocol::Stats {
    let active_duration = app_state.start_time.elapsed();

    let capacity = app_state.total_players.load(Ordering::Relaxed);
    let active_streams = app_state.playing_players.load(Ordering::Relaxed);

    let ram_report = MemoryScanner::examine();
    let cpu_report = compute_cpu_utilization();

    let transmission_report = current_session.and_then(|sess| {
        PacketAuditor::calculate_frame_metrics(sess, app_state.config.server.stats_interval)
    });

    protocol::Stats {
        players: capacity,
        playing_players: active_streams,
        uptime: active_duration.as_millis() as u64,
        memory: protocol::Memory {
            free: ram_report.available,
            used: ram_report.rss,
            allocated: ram_report.rss,
            reservable: ram_report.total,
        },
        cpu: protocol::Cpu {
            cores: cpu_report.core_count,
            system_load: cpu_report.system_wide,
            lavalink_load: cpu_report.process_specific,
        },
        frame_stats: transmission_report,
    }
}

struct MemoryReport {
    rss: u64,
    available: u64,
    total: u64,
}

struct MemoryScanner;

impl MemoryScanner {
    fn examine() -> MemoryReport {
        let rss = Self::read_proc_self_status_rss().unwrap_or(0);
        let (total, available) = Self::read_meminfo().unwrap_or((0, 0));

        MemoryReport {
            rss,
            available,
            total,
        }
    }

    fn read_proc_self_status_rss() -> Option<u64> {
        let content = std::fs::read_to_string("/proc/self/status").ok()?;
        content
            .lines()
            .find(|l| l.starts_with("VmRSS:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .map(|kb| kb * 1024)
    }

    fn read_meminfo() -> Option<(u64, u64)> {
        let content = std::fs::read_to_string("/proc/meminfo").ok()?;
        let mut total = 0;
        let mut available = 0;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total = line.split_whitespace().nth(1)?.parse::<u64>().ok()? * 1024;
            } else if line.starts_with("MemAvailable:") {
                available = line.split_whitespace().nth(1)?.parse::<u64>().ok()? * 1024;
            }
        }
        Some((total, available))
    }
}

struct CpuReport {
    core_count: i32,
    system_wide: f64,
    process_specific: f64,
}

fn compute_cpu_utilization() -> CpuReport {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(1);

    let system_load = SystemCpuSampler::sample().unwrap_or(0.0);
    let process_raw = ProcessCpuSampler::sample().unwrap_or(0.0);

    // Normalize process load by core count to match convention
    let process_load = (process_raw / cores as f64).clamp(0.0, 1.0);

    CpuReport {
        core_count: cores,
        system_wide: system_load,
        process_specific: process_load,
    }
}

struct SystemCpuSampler;

impl SystemCpuSampler {
    fn sample() -> Option<f64> {
        static PREV_IDLE: AtomicU64 = AtomicU64::new(0);
        static PREV_TOTAL: AtomicU64 = AtomicU64::new(0);

        let stat = std::fs::read_to_string("/proc/stat").ok()?;
        let first_line = stat.lines().next()?;
        let fields: Vec<&str> = first_line.split_whitespace().collect();

        if fields.len() < 5 || fields[0] != "cpu" {
            return None;
        }

        let total: u64 = fields[1..]
            .iter()
            .filter_map(|&s| s.parse::<u64>().ok())
            .sum();

        let idle = fields[4].parse::<u64>().ok()?;

        let last_idle = PREV_IDLE.swap(idle, Ordering::Relaxed);
        let last_total = PREV_TOTAL.swap(total, Ordering::Relaxed);

        if last_total == 0 {
            return None;
        }

        let delta_idle = idle.saturating_sub(last_idle);
        let delta_total = total.saturating_sub(last_total);

        if delta_total == 0 {
            return Some(0.0);
        }

        Some(delta_total.saturating_sub(delta_idle) as f64 / delta_total as f64)
    }
}

struct ProcessCpuSampler;

impl ProcessCpuSampler {
    fn sample() -> Option<f64> {
        static PREV_CPU_TICKS: AtomicU64 = AtomicU64::new(0);
        static PREV_WALL_TICKS: AtomicU64 = AtomicU64::new(0);

        // utime is 14th field, stime is 15th in /proc/self/stat
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        let after_comm = stat.rfind(')')?;
        let fields: Vec<&str> = stat[after_comm + 1..].split_whitespace().collect();

        let utime = fields.get(11)?.parse::<u64>().ok()?;
        let stime = fields.get(12)?.parse::<u64>().ok()?;
        let total_cpu_ticks = utime + stime;

        let uptime_str = std::fs::read_to_string("/proc/uptime").ok()?;
        let uptime_secs = uptime_str.split_whitespace().next()?.parse::<f64>().ok()?;

        // Linux USER_HZ is consistently 100
        const HZ: f64 = 100.0;
        let wall_ticks = (uptime_secs * HZ) as u64;

        let last_cpu = PREV_CPU_TICKS.swap(total_cpu_ticks, Ordering::Relaxed);
        let last_wall = PREV_WALL_TICKS.swap(wall_ticks, Ordering::Relaxed);

        if last_wall == 0 {
            return None;
        }

        let delta_cpu = total_cpu_ticks.saturating_sub(last_cpu) as f64;
        let delta_wall = wall_ticks.saturating_sub(last_wall) as f64;

        if delta_wall == 0.0 {
            return Some(0.0);
        }

        Some(delta_cpu / delta_wall)
    }
}

struct PacketAuditor;

impl PacketAuditor {
    fn calculate_frame_metrics(
        session: &crate::server::session::Session,
        interval_secs: u64,
    ) -> Option<protocol::FrameStats> {
        let mut total_delivered = session.total_sent_historical.load(Ordering::Relaxed);
        let mut total_dropped = session.total_nulled_historical.load(Ordering::Relaxed);
        let mut live_players = 0;

        for entry in session.players.iter() {
            if let Ok(p) = entry.value().try_read()
                && p.track.is_some()
                && !p.paused
            {
                live_players += 1;
                total_delivered += p.frames_sent.load(Ordering::Relaxed);
                total_dropped += p.frames_nulled.load(Ordering::Relaxed);
            }
        }

        let previous_delivered = session
            .last_stats_sent
            .swap(total_delivered, Ordering::Relaxed);
        let previous_dropped = session
            .last_stats_nulled
            .swap(total_dropped, Ordering::Relaxed);

        if live_players == 0 {
            return None;
        }

        // Only calculate delta if we have a previous baseline
        if previous_delivered == 0 && previous_dropped == 0 {
            return None;
        }

        let delta_sent = total_delivered.saturating_sub(previous_delivered) as i32;
        let delta_nulled = total_dropped.saturating_sub(previous_dropped) as i32;

        // Expect 50 frames per second per player (standard Opus/Discord frame rate)
        let nominal_total = (interval_secs * 50) as i32 * live_players;
        let gap = nominal_total - (delta_sent + delta_nulled);

        Some(protocol::FrameStats {
            sent: delta_sent / live_players,
            nulled: delta_nulled / live_players,
            deficit: gap / live_players,
        })
    }
}
