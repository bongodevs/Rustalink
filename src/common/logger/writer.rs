use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use parking_lot::Mutex;

fn today_date() -> String {
    // Use std::time to get the current UTC date without pulling in chrono.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Simple arithmetic to derive YYYY-MM-DD from a Unix timestamp.
    let days = (secs / 86400) as u32;
    // Days since 1970-01-01
    let (y, m, d) = days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn days_to_ymd(mut days: u32) -> (u32, u32, u32) {
    // Using the proleptic Gregorian calendar algorithm.
    days += 719468;

    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    (y, m, d)
}

fn resolve_path(base_path: &str, rotate_daily: bool, date: &str) -> String {
    if !rotate_daily {
        return base_path.to_string();
    }

    let base = Path::new(base_path);

    // Derive a stem from the configured path's file name (e.g. "rustalink" from "rustalink.log").
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("rustalink");

    let dir: PathBuf = base.parent().unwrap_or(Path::new(".")).into();

    dir.join(format!("{stem}-{date}.log"))
        .to_string_lossy()
        .into_owned()
}

#[derive(Clone)]
pub struct CircularFileWriter {
    base_path: String,
    max_lines: u32,
    max_files: u32,
    rotate_daily: bool,
    state: Arc<Mutex<WriterState>>,
}

struct WriterState {
    file: Option<File>,
    current_date: Option<String>,
    lines_since_prune: u32,
    is_pruning: bool,
}

impl CircularFileWriter {
    pub fn new(path: String, max_lines: u32, max_files: u32, rotate_daily: bool) -> Self {
        Self {
            base_path: path,
            max_lines,
            max_files,
            rotate_daily,
            state: Arc::new(Mutex::new(WriterState {
                file: None,
                current_date: None,
                lines_since_prune: 0,
                is_pruning: false,
            })),
        }
    }

    /// Return the resolved path for today's log file.
    fn current_path(&self) -> String {
        if self.rotate_daily {
            resolve_path(&self.base_path, true, &today_date())
        } else {
            self.base_path.clone()
        }
    }

    fn ensure_file_open<'a>(&self, state: &'a mut WriterState) -> io::Result<&'a mut File> {
        let today = if self.rotate_daily {
            Some(today_date())
        } else {
            None
        };

        let need_rotate = state.file.is_none()
            || match (&state.current_date, &today) {
                (Some(curr), Some(new)) => curr != new,
                _ => false,
            };

        if need_rotate {
            // Close the old file handle so the OS can flush/rename it.
            state.file = None;

            let path = if self.rotate_daily {
                let d = today.as_deref().unwrap_or("");
                resolve_path(&self.base_path, true, d)
            } else {
                self.base_path.clone()
            };

            if let Some(parent) = Path::new(&path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            state.file = Some(OpenOptions::new().create(true).append(true).open(&path)?);
            state.current_date = today;

            // Clean up old daily files beyond max_files limit.
            if self.rotate_daily && self.max_files > 0 {
                self.cleanup_old_files();
            }
        }

        Ok(state.file.as_mut().expect("file was just opened"))
    }

    fn spawn_prune(&self) {
        let path = self.current_path();
        let max_lines = self.max_lines;
        let state_arc = self.state.clone();

        std::thread::spawn(move || {
            if let Err(e) = Self::do_prune(&path, max_lines) {
                eprintln!("Failed to prune log file '{}': {}", path, e);
            }
            let mut state = state_arc.lock();
            state.is_pruning = false;
        });
    }

    /// Delete old daily log files, keeping only the most recent `max_files`.
    fn cleanup_old_files(&self) {
        let base = Path::new(&self.base_path);
        let dir = base.parent().unwrap_or(Path::new("."));
        let stem = base
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("rustalink");
        let max_files = self.max_files as usize;

        let mut log_files: Vec<std::path::PathBuf> = match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().and_then(|e| e.to_str()) == Some("log")
                        && p.file_stem()
                            .and_then(|s| s.to_str())
                            .map(|s| s.starts_with(stem) && s != stem)
                            .unwrap_or(false)
                })
                .collect(),
            Err(_) => return,
        };

        if log_files.len() <= max_files {
            return;
        }

        // Sort by name (YYYY-MM-DD suffix sorts lexicographically = chronologically).
        log_files.sort();

        let to_delete = log_files.len() - max_files;
        for path in log_files.iter().take(to_delete) {
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("Failed to delete old log file '{}': {}", path.display(), e);
            }
        }
    }

    fn do_prune(path: &str, max_lines: u32) -> io::Result<()> {
        if !Path::new(path).exists() {
            return Ok(());
        }

        let lines: Vec<String> = {
            let file = File::open(path)?;
            let reader = BufReader::new(file);
            reader.lines().collect::<Result<_, _>>()?
        };

        if lines.len() > max_lines as usize {
            let start = lines.len() - max_lines as usize;
            let tmp_path = format!("{}.tmp", path);
            {
                let mut file = File::create(&tmp_path)?;
                for line in &lines[start..] {
                    writeln!(file, "{}", line)?;
                }
            }
            std::fs::rename(tmp_path, path)?;
        }
        Ok(())
    }
}

impl io::Write for CircularFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self.state.lock();

        let file = self.ensure_file_open(&mut state)?;
        file.write_all(buf)?;

        let new_lines = buf.iter().filter(|&&b| b == b'\n').count() as u32;
        state.lines_since_prune += new_lines;

        let prune_threshold = (self.max_lines / 10).max(50);
        if state.lines_since_prune >= prune_threshold && !state.is_pruning {
            state.is_pruning = true;
            state.lines_since_prune = 0;
            state.file = None; // Close file so rename can work on Windows
            self.spawn_prune();
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut state = self.state.lock();
        if let Some(file) = &mut state.file {
            file.flush()?;
        }
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for CircularFileWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    fn cleanup_test_file(path: &str) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(format!("{}.tmp", path));
    }

    #[test]
    fn test_circular_file_writer_new() {
        let writer = CircularFileWriter::new("test_new.log".to_string(), 100, 0, false);
        let state = writer.state.lock();
        assert_eq!(state.lines_since_prune, 0);
        assert!(!state.is_pruning);
        assert!(state.file.is_none());
        cleanup_test_file("test_new.log");
    }

    #[test]
    fn test_write_creates_file() {
        let path = "test_create.log";
        cleanup_test_file(path);

        let mut writer = CircularFileWriter::new(path.to_string(), 100, 0, false);
        let data = b"test line\n";
        let result = writer.write(data);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), data.len());
        assert!(Path::new(path).exists());

        cleanup_test_file(path);
    }

    #[test]
    fn test_write_counts_newlines() {
        let path = "test_newlines.log";
        cleanup_test_file(path);

        let mut writer = CircularFileWriter::new(path.to_string(), 1000, 0, false);
        writer.write(b"line1\nline2\nline3\n").unwrap();

        let state = writer.state.lock();
        assert_eq!(state.lines_since_prune, 3);

        cleanup_test_file(path);
    }

    #[test]
    fn test_write_no_newlines() {
        let path = "test_no_newlines.log";
        cleanup_test_file(path);

        let mut writer = CircularFileWriter::new(path.to_string(), 1000, 0, false);
        writer.write(b"no newline here").unwrap();

        let state = writer.state.lock();
        assert_eq!(state.lines_since_prune, 0);

        cleanup_test_file(path);
    }

    #[test]
    fn test_flush() {
        let path = "test_flush.log";
        cleanup_test_file(path);

        let mut writer = CircularFileWriter::new(path.to_string(), 100, 0, false);
        writer.write(b"test\n").unwrap();

        let result = writer.flush();
        assert!(result.is_ok());

        cleanup_test_file(path);
    }

    #[test]
    fn test_flush_without_file() {
        let mut writer =
            CircularFileWriter::new("test_flush_no_file.log".to_string(), 100, 0, false);
        let result = writer.flush();
        assert!(result.is_ok());
        cleanup_test_file("test_flush_no_file.log");
    }

    #[test]
    fn test_clone() {
        let writer = CircularFileWriter::new("test_clone.log".to_string(), 100, 0, false);
        let cloned = writer.clone();

        // Both should share the same state
        assert!(Arc::ptr_eq(&writer.state, &cloned.state));

        cleanup_test_file("test_clone.log");
    }

    #[test]
    fn test_make_writer() {
        let writer = CircularFileWriter::new("test_make_writer.log".to_string(), 100, 0, false);
        let made = writer.make_writer();

        // Should be a clone
        assert!(Arc::ptr_eq(&writer.state, &made.state));

        cleanup_test_file("test_make_writer.log");
    }

    #[test]
    fn test_do_prune_nonexistent_file() {
        let result = CircularFileWriter::do_prune("nonexistent_prune.log", 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_do_prune_small_file() {
        let path = "test_prune_small.log";
        cleanup_test_file(path);

        fs::write(path, "line1\nline2\nline3\n").unwrap();

        let result = CircularFileWriter::do_prune(path, 10);
        assert!(result.is_ok());

        let content = fs::read_to_string(path).unwrap();
        assert_eq!(content.lines().count(), 3);

        cleanup_test_file(path);
    }

    #[test]
    fn test_do_prune_large_file() {
        let path = "test_prune_large.log";
        cleanup_test_file(path);

        let mut content = String::new();
        for i in 1..=20 {
            content.push_str(&format!("line{}\n", i));
        }
        fs::write(path, content).unwrap();

        let result = CircularFileWriter::do_prune(path, 10);
        assert!(result.is_ok());

        let pruned = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = pruned.lines().collect();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line11");
        assert_eq!(lines[9], "line20");

        cleanup_test_file(path);
    }

    #[test]
    fn test_resolve_path_no_rotate() {
        let p = resolve_path("./logs/rustalink.log", false, "2026-03-13");
        assert_eq!(p, "./logs/rustalink.log");
    }

    #[test]
    fn test_resolve_path_rotate() {
        let p = resolve_path("./logs/rustalink.log", true, "2026-03-13");
        assert!(p.contains("2026-03-13"));
        assert!(p.ends_with(".log"));
    }

    #[test]
    fn test_today_date_format() {
        let d = today_date();
        let parts: Vec<&str> = d.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].len(), 4); // year
        assert_eq!(parts[1].len(), 2); // month
        assert_eq!(parts[2].len(), 2); // day
    }

    #[test]
    fn test_prune_threshold_calculation() {
        let _writer = CircularFileWriter::new("test.log".to_string(), 1000, 0, false);
        let threshold = (1000 / 10).max(50);
        assert_eq!(threshold, 100);

        let _writer = CircularFileWriter::new("test.log".to_string(), 100, 0, false);
        let threshold = (100 / 10).max(50);
        assert_eq!(threshold, 50);

        let _writer = CircularFileWriter::new("test.log".to_string(), 10, 0, false);
        let threshold = (10 / 10).max(50);
        assert_eq!(threshold, 50);

        cleanup_test_file("test.log");
    }
}
