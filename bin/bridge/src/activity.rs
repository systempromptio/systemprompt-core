//! In-memory ring-buffer activity log with persistent file rotation and emit
//! hooks for the GUI.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

const LOG_CAPACITY: usize = 1000;
const PERSISTENT_MAX_BYTES: u64 = 10 * 1024 * 1024;

pub type EmitHook = Box<dyn Fn(&LogEntry) + Send + Sync>;

#[expect(
    missing_debug_implementations,
    reason = "holds Vec<Box<dyn Fn(&LogEntry) + Send + Sync>> hooks; cannot derive Debug"
)]
#[derive(Clone)]
pub struct ActivityLog {
    inner: Arc<Mutex<LogState>>,
    hooks: Arc<Mutex<Vec<EmitHook>>>,
    persistent: Arc<OnceLock<PersistentWriter>>,
    persistence_error: Arc<Mutex<Option<String>>>,
}

struct LogState {
    next_id: u64,
    entries: VecDeque<LogEntry>,
}

/// Severity carried on every activity line so the GUI never has to guess it
/// from the wording.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    #[default]
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct LogEntry {
    pub id: u64,
    pub ts_unix: u64,
    pub level: LogLevel,
    pub line: String,
}

impl Default for ActivityLog {
    fn default() -> Self {
        Self::new()
    }
}

impl ActivityLog {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(LogState {
                next_id: 1,
                entries: VecDeque::with_capacity(LOG_CAPACITY),
            })),
            hooks: Arc::new(Mutex::new(Vec::new())),
            persistent: Arc::new(OnceLock::new()),
            persistence_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn append(&self, line: impl Into<String>) {
        self.append_at(LogLevel::Info, line);
    }

    pub fn append_warn(&self, line: impl Into<String>) {
        self.append_at(LogLevel::Warn, line);
    }

    pub fn append_error(&self, line: impl Into<String>) {
        self.append_at(LogLevel::Error, line);
    }

    pub fn append_at(&self, level: LogLevel, line: impl Into<String>) {
        let mut entry = {
            let mut g = self.inner.lock();
            let id = g.next_id;
            g.next_id += 1;
            let entry = LogEntry {
                id,
                ts_unix: now_unix(),
                level,
                line: line.into(),
            };
            if g.entries.len() == LOG_CAPACITY {
                g.entries.pop_front();
            }
            g.entries.push_back(entry.clone());
            entry
        };
        if let Some(writer) = self.persistent.get() {
            let written = serde_json::to_string(&entry)
                .map_err(std::io::Error::other)
                .and_then(|line| writer.write(&line));
            if let Err(e) = written {
                let message = format!("activity log {}: {e}", writer.path.display());
                *self.persistence_error.lock() = Some(message.clone());
                entry.level = LogLevel::Error;
                entry.line = format!("{message}; event was not persisted: {}", entry.line);
                if let Some(stored) = self
                    .inner
                    .lock()
                    .entries
                    .iter_mut()
                    .find(|stored| stored.id == entry.id)
                {
                    *stored = entry.clone();
                }
            }
        }
        let hooks = self.hooks.lock();
        for hook in hooks.iter() {
            hook(&entry);
        }
    }

    pub fn ensure_persistence(&self) -> std::io::Result<()> {
        self.persistence_error
            .lock()
            .as_ref()
            .map_or(Ok(()), |error| Err(std::io::Error::other(error.clone())))
    }

    pub fn snapshot_since(&self, since: u64) -> Vec<LogEntry> {
        let g = self.inner.lock();
        g.entries.iter().filter(|e| e.id > since).cloned().collect()
    }

    pub fn snapshot_recent(&self, limit: usize) -> Vec<LogEntry> {
        let g = self.inner.lock();
        let len = g.entries.len();
        let start = len.saturating_sub(limit);
        g.entries.iter().skip(start).cloned().collect()
    }

    pub fn add_emit_hook(&self, hook: EmitHook) {
        self.hooks.lock().push(hook);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[must_use]
fn jsonl_path() -> Option<PathBuf> {
    crate::obs::log_dir().map(|d| d.join("activity.jsonl"))
}

#[must_use]
fn jsonl_rolled_path() -> Option<PathBuf> {
    crate::obs::log_dir().map(|d| d.join("activity.jsonl.1"))
}

struct PersistentWriter {
    path: PathBuf,
    rolled: PathBuf,
    file: Mutex<BufWriter<File>>,
    bytes: AtomicU64,
}

impl PersistentWriter {
    fn open(path: PathBuf, rolled: PathBuf) -> std::io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            path,
            rolled,
            file: Mutex::new(BufWriter::new(file)),
            bytes: AtomicU64::new(bytes),
        })
    }

    fn write(&self, line: &str) -> std::io::Result<()> {
        {
            let mut guard = self.file.lock();
            writeln!(guard, "{line}")?;
            guard.flush()?;
        }
        let new_bytes = self
            .bytes
            .fetch_add(line.len() as u64 + 1, Ordering::Relaxed)
            + line.len() as u64
            + 1;
        if new_bytes > PERSISTENT_MAX_BYTES {
            self.rollover()?;
        }
        Ok(())
    }

    fn rollover(&self) -> std::io::Result<()> {
        let mut guard = self.file.lock();
        guard.flush()?;
        match std::fs::remove_file(&self.rolled) {
            Ok(()) => {},
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
            Err(e) => return Err(e),
        }
        std::fs::rename(&self.path, &self.rolled)?;
        *guard = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        );
        drop(guard);
        self.bytes.store(0, Ordering::Relaxed);
        Ok(())
    }
}

pub fn install_persistent_writer(log: &ActivityLog) -> std::io::Result<()> {
    let path =
        jsonl_path().ok_or_else(|| std::io::Error::other("activity log path unresolvable"))?;
    let rolled = jsonl_rolled_path()
        .ok_or_else(|| std::io::Error::other("activity rollover path unresolvable"))?;
    let writer = PersistentWriter::open(path, rolled)?;
    log.persistent.set(writer).map_err(|writer| {
        std::io::Error::other(format!(
            "activity writer already installed for {}",
            writer.path.display()
        ))
    })
}
