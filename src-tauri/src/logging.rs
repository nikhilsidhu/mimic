//! Local diagnostics: a daily log file in the data directory and a panic hook. Nothing
//! leaves the machine.

use std::path::Path;
use std::sync::Mutex;

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;

/// Days of logs kept.
const KEEP_FILES: usize = 7;

/// Starts logging to `<logs_dir>/mimic.<date>.log`. `MIMIC_LOG` overrides the
/// level filter, e.g. `MIMIC_LOG=mimic_lib=trace`.
pub fn init(logs_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    std::fs::create_dir_all(logs_dir)?;
    let file = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("mimic")
        .filename_suffix("log")
        .max_log_files(KEEP_FILES)
        .build(logs_dir)?;

    let filter = EnvFilter::try_from_env("MIMIC_LOG").unwrap_or_else(|_| EnvFilter::new("info,mimic_lib=debug"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        // Written synchronously: volume is low, and a panic must reach the file even
        // though release builds abort straight after the hook.
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .try_init()?;

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!("panic: {info}\n{backtrace}");
        default_hook(info);
    }));

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "mimic started");
    Ok(())
}
