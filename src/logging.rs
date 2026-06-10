//! Process-wide diagnostics logging.
//!
//! Installs ONE global `tracing` subscriber early in `main` that fans every
//! event to (a) a daily on-disk log file under `%APPDATA%\Polaris\logs\` and
//! (b) an in-memory ring buffer the Diagnostics panel tails. Because we install
//! first, EasyTier's own `try_init` no-ops and its events (hole-punch attempts,
//! peer connections, …) flow here too.
//!
//! The level is reloadable at runtime (Settings → Diagnostics) through a
//! `reload::Handle`; when diagnostics are disabled the filter is set to `off`,
//! so no events are processed and no log file is created.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, fmt, reload};

use crate::config::LogLevel;

/// Most recent lines kept in memory for the panel.
const RING_CAP: usize = 4000;

static RING: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static FILE: Mutex<Option<File>> = Mutex::new(None);
static RELOAD: OnceLock<reload::Handle<EnvFilter, Registry>> = OnceLock::new();

/// `%APPDATA%\Polaris\logs`.
pub fn logs_dir() -> PathBuf {
    let mut p = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    p.push("Polaris");
    p.push("logs");
    p
}

fn env_filter(level: LogLevel) -> EnvFilter {
    match level {
        LogLevel::Off => EnvFilter::new("off"),
        // Keep noisy third-party crates at warn; trace our app + the engine.
        l => EnvFilter::new(format!(
            "warn,polaris_et={f},easytier={f}",
            f = l.as_filter()
        )),
    }
}

/// Install the global subscriber. Call once, before starting any network.
pub fn init(level: LogLevel) {
    let (filter, handle) = reload::Layer::new(env_filter(level));
    let _ = RELOAD.set(handle);

    let layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_timer(fmt::time::ChronoLocal::new(
            "%Y-%m-%d %H:%M:%S%.3f".to_string(),
        ))
        .with_writer(|| Fanout);

    // `try_init` (not `init`) so a stray pre-existing subscriber can't panic us.
    let _ = Registry::default().with(filter).with(layer).try_init();
}

/// Change the active level at runtime. No-op before [`init`].
pub fn set_level(level: LogLevel) {
    if let Some(h) = RELOAD.get() {
        let _ = h.reload(env_filter(level));
    }
}

/// Newest `n` log lines, oldest first.
pub fn recent(n: usize) -> Vec<String> {
    RING.lock()
        .map(|r| {
            let skip = r.len().saturating_sub(n);
            r.iter().skip(skip).cloned().collect()
        })
        .unwrap_or_default()
}

/// Drop the in-memory buffer (does not touch the log files).
pub fn clear() {
    if let Ok(mut r) = RING.lock() {
        r.clear();
    }
}

/// Delete `*.log` files older than `retention_days` (by mtime). The file
/// currently being written is freshly modified, so it is always kept.
pub fn cleanup(retention_days: u32) {
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(u64::from(retention_days) * 86_400))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let Ok(entries) = fs::read_dir(logs_dir()) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|m| m < cutoff);
        if old && p.extension().is_some_and(|x| x == "log") {
            let _ = fs::remove_file(&p);
        }
    }
}

/// Write a single combined dump (`header` + every on-disk log file) to `dest`.
pub fn export(dest: &Path, header: &str) -> io::Result<()> {
    let mut out = File::create(dest)?;
    writeln!(out, "{header}\n")?;
    let mut files: Vec<PathBuf> = fs::read_dir(logs_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "log"))
        .collect();
    files.sort();
    for f in &files {
        let name = f
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        writeln!(out, "\n===== {name} =====")?;
        if let Ok(content) = fs::read_to_string(f) {
            out.write_all(content.as_bytes())?;
        }
    }
    out.flush()
}

/// Open (once, lazily) today's append-mode log file.
fn open_today() -> Option<File> {
    let dir = logs_dir();
    fs::create_dir_all(&dir).ok()?;
    let name = format!("polaris-{}.log", chrono::Local::now().format("%Y-%m-%d"));
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(name))
        .ok()
}

fn push_ring(s: &str) {
    if let Ok(mut ring) = RING.lock() {
        for line in s.split_inclusive('\n') {
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                continue;
            }
            if ring.len() >= RING_CAP {
                ring.pop_front();
            }
            ring.push_back(line.to_string());
        }
    }
}

/// `Write` sink the fmt layer renders each event into: appends to the day's log
/// file (opened lazily on first event) and the in-memory ring.
struct Fanout;

impl Write for Fanout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(mut g) = FILE.lock() {
            if g.is_none() {
                *g = open_today();
            }
            if let Some(f) = g.as_mut() {
                let _ = f.write_all(buf);
            }
        }
        push_ring(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Ok(mut g) = FILE.lock()
            && let Some(f) = g.as_mut()
        {
            let _ = f.flush();
        }
        Ok(())
    }
}
