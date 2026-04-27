use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, Once};

use log::{LevelFilter, Log, Metadata, Record};

/// File-based logger for desktop release builds where stdout is unavailable.
///
/// Writes to `%LOCALAPPDATA%/com.mu.realtime-translator/logs/realtime-translator.log`
/// in append mode. Also prints to stderr in debug builds.
struct FileLogger {
    file: Mutex<Option<std::fs::File>>,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let level = record.level();
        let module = record.module_path().unwrap_or("unknown");
        let message = record.args();

        let line = format!("[{} {} {}] {}\n", now, level, module, message);

        // Write to file.
        if let Ok(mut guard) = self.file.lock() {
            if let Some(ref mut file) = *guard {
                if file.write_all(line.as_bytes()).is_err() {
                    #[cfg(debug_assertions)]
                    eprint!("[Logger] write failed: {}", line.trim());
                }
            }
        }

        // Also print to stderr in debug builds.
        #[cfg(debug_assertions)]
        {
            eprint!("{}", line);
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock() {
            if let Some(ref mut file) = *guard {
                let _ = file.flush();
            }
        }
    }
}

static LOGGER: FileLogger = FileLogger { file: Mutex::new(None) };
static INIT: Once = Once::new();

/// Initialize the file logger.
///
/// Creates the log directory if it doesn't exist, opens the log file in append mode,
/// and sets the global logger. Safe to call multiple times — subsequent calls are no-ops.
pub fn init() {
    INIT.call_once(|| {
        let log_path = match log_dir() {
            Some(path) => path.join("realtime-translator.log"),
            None => {
                eprintln!("[Logger] Could not determine log directory, logging disabled");
                return;
            }
        };

        // Create log directory if needed.
        if let Some(parent) = log_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("[Logger] Failed to create log dir {:?}: {}", parent, e);
                return;
            }
        }

        // Open log file in append mode.
        match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(f) => {
                if let Ok(mut guard) = LOGGER.file.lock() {
                    *guard = Some(f);
                }
            }
            Err(e) => {
                eprintln!("[Logger] Failed to open {:?}: {}", log_path, e);
                return;
            }
        }

        // Determine max level.
        let max_level = resolve_max_level();

        if let Err(e) = log::set_logger(&LOGGER) {
            eprintln!("[Logger] set_logger failed: {}", e);
            return;
        }
        log::set_max_level(max_level);

        log::info!("Logger initialized (level={:?}, path={:?})", max_level, log_path);
    });
}

/// Determine the log level filter.
///
/// 1. Check `RUST_LOG` env var for override.
/// 2. Debug builds → Debug level.
/// 3. Release builds → Info level (debug!/trace! are compiled out via features).
fn resolve_max_level() -> LevelFilter {
    if let Ok(env) = std::env::var("RUST_LOG") {
        return match env.parse::<LevelFilter>() {
            Ok(level) => level,
            Err(_) => {
                eprintln!("[Logger] Invalid RUST_LOG value: {:?}, using Info", env);
                LevelFilter::Info
            }
        };
    }

    if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    }
}

/// Get the log directory: `%LOCALAPPDATA%/com.mu.realtime-translator/logs`
fn log_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("com.mu.realtime-translator").join("logs"))
}
