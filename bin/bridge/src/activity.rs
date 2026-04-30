use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;

const LOG_CAPACITY: usize = 1000;

#[derive(Clone)]
pub struct ActivityLog {
    inner: Arc<Mutex<LogState>>,
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
        }
    }

    pub fn append(&self, line: impl Into<String>) {
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
        g.entries.push_back(entry);
    }

    pub fn snapshot_since(&self, since: u64) -> Vec<LogEntry> {
        let g = self.inner.lock();
        g.entries.iter().filter(|e| e.id > since).cloned().collect()
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
