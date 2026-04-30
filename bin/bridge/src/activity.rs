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

#[derive(Clone)]
pub struct ActivityLog {
    inner: Arc<Mutex<LogState>>,
    hooks: Arc<Mutex<Vec<EmitHook>>>,
}

struct LogState {
    next_id: u64,
    entries: VecDeque<LogEntry>,
}

#[derive(Clone, serde::Serialize)]
pub struct LogEntry {
    pub id: u64,
    pub ts_unix: u64,
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
        }
    }

    pub fn append(&self, line: impl Into<String>) {
        let entry = {
            let mut g = self.inner.lock();
            let id = g.next_id;
            g.next_id += 1;
            let entry = LogEntry {
                id,
                ts_unix: now_unix(),
                line: line.into(),
            };
            if g.entries.len() == LOG_CAPACITY {
                g.entries.pop_front();
            }
            g.entries.push_back(entry.clone());
            entry
        };
        let hooks = self.hooks.lock();
        for hook in hooks.iter() {
            hook(&entry);
        }
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

static ACTIVITY_LOG: OnceLock<ActivityLog> = OnceLock::new();

pub fn activity_log() -> &'static ActivityLog {
    ACTIVITY_LOG.get_or_init(ActivityLog::new)
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
        let bytes = file.metadata().map(|m| m.len()).unwrap_or(0);
        Ok(Self {
            path,
            rolled,
            file: Mutex::new(BufWriter::new(file)),
            bytes: AtomicU64::new(bytes),
        })
    }

    fn write(&self, line: &str) {
        {
            let mut guard = self.file.lock();
            if writeln!(guard, "{line}").is_ok() {
                let _ = guard.flush();
            }
        }
        let new_bytes = self
            .bytes
            .fetch_add(line.len() as u64 + 1, Ordering::Relaxed)
            + line.len() as u64
            + 1;
        if new_bytes > PERSISTENT_MAX_BYTES {
            self.try_rollover();
        }
    }

    fn try_rollover(&self) {
        let mut guard = self.file.lock();
        let _ = guard.flush();
        let _ = std::fs::remove_file(&self.rolled);
        if std::fs::rename(&self.path, &self.rolled).is_err() {
            return;
        }
        let new_file = OpenOptions::new()
            .create(true)
            .append(true)
            .truncate(false)
            .open(&self.path);
        if let Ok(f) = new_file {
            *guard = BufWriter::new(f);
            self.bytes.store(0, Ordering::Relaxed);
        }
    }
}

pub fn install_persistent_writer() {
    let Some(path) = jsonl_path() else {
        return;
    };
    let Some(rolled) = jsonl_rolled_path() else {
        return;
    };
    let writer = match PersistentWriter::open(path, rolled) {
        Ok(w) => Arc::new(w),
        Err(e) => {
            tracing::warn!(error = %e, "activity: persistent writer disabled");
            return;
        },
    };
    activity_log().add_emit_hook(Box::new(move |entry| {
        let line = match serde_json::to_string(entry) {
            Ok(s) => s,
            Err(_) => return,
        };
        writer.write(&line);
    }));
}
