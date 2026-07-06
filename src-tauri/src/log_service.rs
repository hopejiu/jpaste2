use once_cell::sync::OnceCell;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Log levels matching the log crate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

impl Level {
    fn as_str(&self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

/// LogService writes logs to a daily-hourly file and optionally to stderr.
/// Uses Arc<Mutex<LogWriter>> shared across all clones for proper mutual exclusion.
pub struct LogService {
    log_dir: PathBuf,
    writer: Arc<Mutex<LogWriter>>,
    retention_hours: u64,
}

struct LogWriter {
    file: Option<File>,
    current_pattern: String,
}

impl LogService {
    /// Create and initialize the log service.
    /// Creates the log directory and opens the initial log file.
    /// Starts background rotation + cleanup + flush.
    pub fn init(app_data: &Path, retention_hours: u64) -> Result<Self, String> {
        let log_dir = app_data.join("logs");
        fs::create_dir_all(&log_dir).map_err(|e| format!("create log dir: {}", e))?;

        let (file, pattern) = open_current_log(&log_dir)?;
        let svc = Self {
            log_dir,
            writer: Arc::new(Mutex::new(LogWriter {
                file: Some(file),
                current_pattern: pattern,
            })),
            retention_hours,
        };

        // Spawn background rotation + cleanup + flush
        let svc_clone = svc.try_clone().expect("clone for bg task");
        std::thread::spawn(move || {
            let mut counter: u32 = 0;
            loop {
                std::thread::sleep(Duration::from_secs(5));
                // Flush every 5 seconds (replaces per-write sync_all)
                if let Ok(mut w) = svc_clone.writer.lock() {
                    if let Some(ref mut f) = w.file {
                        let _ = f.flush();
                    }
                }
                // Rotate and cleanup every 5 minutes (60 iterations)
                counter += 1;
                if counter >= 60 {
                    counter = 0;
                    if let Err(e) = svc_clone.rotate() {
                        eprintln!("[log] rotate error: {}", e);
                    }
                    if let Err(e) = svc_clone.cleanup() {
                        eprintln!("[log] cleanup error: {}", e);
                    }
                }
            }
        });

        Ok(svc)
    }

    /// Write a log line (no fsync — flush happens every 5s in background)
    pub fn write(&self, level: Level, target: &str, msg: &str) {
        let now = compact_now();
        let line = format!(
            "{} [{:5}] {}: {}\n",
            now,
            level.as_str(),
            target,
            msg
        );
        let line_bytes = line.as_bytes();

        if let Ok(mut w) = self.writer.lock() {
            if let Some(ref mut f) = w.file {
                // No sync_all() here — flush is batched every 5s.
                // Ceiling: up to 5s of logs lost on hard crash/BSOD.
                let _ = f.write_all(line_bytes);
            }
        }
        // Always write to stderr for `tauri dev`
        let _ = io::stderr().write_all(line_bytes);
    }

    /// Forward a frontend log message
    pub fn frontend_log(&self, level: &str, component: &str, msg: &str) {
        let lvl = match level {
            "debug" | "DEBUG" => Level::Debug,
            "warn" | "WARN" => Level::Warn,
            "error" | "ERROR" => Level::Error,
            _ => Level::Info,
        };
        self.write(lvl, &format!("[FE:{}]", component), msg);
    }

    /// Rotate log file if hour has changed
    fn rotate(&self) -> Result<(), String> {
        let (new_file, new_pattern) = open_current_log(&self.log_dir)?;
        let mut w = self.writer.lock().map_err(|e| e.to_string())?;
        if new_pattern != w.current_pattern {
            // Flush old file before switching
            if let Some(ref mut f) = w.file {
                let _ = f.flush();
            }
            w.file = Some(new_file);
            w.current_pattern = new_pattern;
        }
        Ok(())
    }

    /// Remove log files older than retention_hours
    fn cleanup(&self) -> Result<(), String> {
        let cutoff = SystemTime::now()
            - Duration::from_secs(self.retention_hours * 3600);

        let entries = fs::read_dir(&self.log_dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("log") {
                continue;
            }
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < cutoff {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
        Ok(())
    }

    /// Clone for background thread / log bridge — shares the same writer.
    fn try_clone(&self) -> Result<Self, String> {
        Ok(Self {
            log_dir: self.log_dir.clone(),
            writer: Arc::clone(&self.writer),
            retention_hours: self.retention_hours,
        })
    }
}

fn open_current_log(log_dir: &Path) -> Result<(File, String), String> {
    let pattern = compact_now_hour(); // "2026-07-03-12"
    let path = log_dir.join(format!("jpaste-{}.log", pattern));

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("open log file: {}", e))?;

    Ok((file, pattern))
}

/// Compact timestamp: "2026-07-03 12:30:15.123" — local time via chrono.
fn compact_now() -> String {
    let now = chrono::Local::now();
    format!("{}", now.format("%Y-%m-%d %H:%M:%S%.3f"))
}

/// Compact hour pattern: "2026-07-03-12" — local time via chrono.
fn compact_now_hour() -> String {
    chrono::Local::now().format("%Y-%m-%d-%H").to_string()
}

// ── Log crate bridge: redirects `log::info!()` etc. to LogService ──────

struct LogBridge {
    svc: LogService,
}

impl LogBridge {
    fn new(svc: LogService) -> Self {
        Self { svc }
    }
}

impl log::Log for LogBridge {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let level = match record.level() {
            log::Level::Error => Level::Error,
            log::Level::Warn => Level::Warn,
            log::Level::Debug => Level::Debug,
            _ => Level::Info,
        };
        let target = record.target();
        let msg = record.args().to_string();
        self.svc.write(level, target, &msg);
    }

    fn flush(&self) {}
}

// ── Global logger storage (once_cell instead of Box::leak) ────────────

static GLOBAL_LOGGER: OnceCell<LogBridge> = OnceCell::new();

/// Initialize global logger: install LogBridge as the log crate's logger.
/// Call this once at startup.
pub fn init_global_logger(app_data: &Path, retention_hours: u64) -> LogService {
    let svc = LogService::init(app_data, retention_hours)
        .expect("Failed to initialize log service");

    let bridge = LogBridge::new(
        svc.try_clone().expect("clone for log bridge"),
    );

    // once_cell::set is safe and doesn't leak memory
    let _ = GLOBAL_LOGGER.set(bridge);
    log::set_logger(GLOBAL_LOGGER.get().expect("logger just initialized"))
        .map(|()| log::set_max_level(log::LevelFilter::Debug))
        .expect("Failed to set global logger");

    svc
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_service_init_creates_dir() {
        let dir = TempDir::new().unwrap();
        let log_dir = dir.path().join("logs");
        let _svc = LogService::init(dir.path(), 24).unwrap();
        assert!(log_dir.exists());
    }

    #[test]
    fn test_log_service_write_creates_file() {
        let dir = TempDir::new().unwrap();
        let svc = LogService::init(dir.path(), 24).unwrap();
        svc.write(Level::Info, "test_target", "hello world");
        // Manually flush for test
        {
            let mut w = svc.writer.lock().unwrap();
            if let Some(ref mut f) = w.file {
                let _ = f.flush();
            }
        }

        // Check that a log file was created
        let log_files: Vec<_> = std::fs::read_dir(dir.path().join("logs"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("log"))
            .collect();
        assert!(!log_files.is_empty(), "log file should be created");
    }

    #[test]
    fn test_log_service_write_content() {
        let dir = TempDir::new().unwrap();
        let svc = LogService::init(dir.path(), 24).unwrap();
        svc.write(Level::Error, "my_module", "test error message");
        // Manually flush for test
        {
            let mut w = svc.writer.lock().unwrap();
            if let Some(ref mut f) = w.file {
                let _ = f.flush();
            }
        }

        // Read the log file and verify content
        let log_content = {
            let log_files: Vec<_> = std::fs::read_dir(dir.path().join("logs"))
                .unwrap()
                .filter_map(|e| e.ok())
                .collect();
            assert_eq!(log_files.len(), 1);
            std::fs::read_to_string(&log_files[0].path()).unwrap()
        };
        assert!(log_content.contains("ERROR"));
        assert!(log_content.contains("my_module"));
        assert!(log_content.contains("test error message"));
    }

    #[test]
    fn test_log_service_frontend_log() {
        let dir = TempDir::new().unwrap();
        let svc = LogService::init(dir.path(), 24).unwrap();
        svc.frontend_log("warn", "frontend-comp", "something happened");
        // Manually flush for test
        {
            let mut w = svc.writer.lock().unwrap();
            if let Some(ref mut f) = w.file {
                let _ = f.flush();
            }
        }

        let log_content = {
            let log_files: Vec<_> = std::fs::read_dir(dir.path().join("logs"))
                .unwrap()
                .filter_map(|e| e.ok())
                .collect();
            std::fs::read_to_string(&log_files[0].path()).unwrap()
        };
        assert!(log_content.contains("WARN"));
        assert!(log_content.contains("[FE:frontend-comp]"));
        assert!(log_content.contains("something happened"));
    }

    #[test]
    fn test_log_service_multiple_levels() {
        let dir = TempDir::new().unwrap();
        let svc = LogService::init(dir.path(), 24).unwrap();
        svc.write(Level::Debug, "t1", "debug msg");
        svc.write(Level::Info, "t2", "info msg");
        svc.write(Level::Warn, "t3", "warn msg");
        svc.write(Level::Error, "t4", "error msg");
        // Manually flush for test
        {
            let mut w = svc.writer.lock().unwrap();
            if let Some(ref mut f) = w.file {
                let _ = f.flush();
            }
        }

        let log_content = {
            let log_files: Vec<_> = std::fs::read_dir(dir.path().join("logs"))
                .unwrap()
                .filter_map(|e| e.ok())
                .collect();
            std::fs::read_to_string(&log_files[0].path()).unwrap()
        };
        assert!(log_content.contains("DEBUG"));
        assert!(log_content.contains("INFO"));
        assert!(log_content.contains("WARN"));
        assert!(log_content.contains("ERROR"));
    }

    #[test]
    fn test_log_level_as_str() {
        assert_eq!(Level::Debug.as_str(), "DEBUG");
        assert_eq!(Level::Info.as_str(), "INFO");
        assert_eq!(Level::Warn.as_str(), "WARN");
        assert_eq!(Level::Error.as_str(), "ERROR");
    }

    #[test]
    fn test_compact_now_format() {
        let now = compact_now();
        // Should match format: "YYYY-MM-DD HH:MM:SS.mmm"
        assert!(now.contains('-'));
        assert!(now.contains(':'));
        assert!(now.contains('.'));
    }

    #[test]
    fn test_compact_now_hour_format() {
        let hour = compact_now_hour();
        // Should match format: "YYYY-MM-DD-HH"
        assert!(hour.contains('-'));
        assert_eq!(hour.matches('-').count(), 3);
    }
}
