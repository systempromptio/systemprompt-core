use std::collections::VecDeque;
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use parking_lot::Mutex;

use crate::gui::connection::{ConnectionContext, handle_connection};
use crate::gui::events::UiEvent;
use crate::gui::server_util::{mint_csrf_token, now_unix};
use crate::gui::state::AppState;
use crate::obs::output::diag;

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
pub(crate) struct LogEntry {
    id: u64,
    ts_unix: u64,
    line: String,
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

    pub(crate) fn snapshot_since(&self, since: u64) -> Vec<LogEntry> {
        let g = self.inner.lock();
        g.entries.iter().filter(|e| e.id > since).cloned().collect()
    }
}

#[derive(Clone)]
pub struct Server {
    port: u16,
    csrf_token: String,
    log: ActivityLog,
}

impl Server {
    #[tracing::instrument(skip(state, tx))]
    pub fn start(state: Arc<AppState>, tx: Sender<UiEvent>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let csrf_token = mint_csrf_token();
        let log = ActivityLog::new();
        tracing::info!(port, "gui-server listening");

        let csrf_clone = csrf_token.clone();
        let log_clone = log.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let stream = match conn {
                    Ok(s) => s,
                    Err(e) => {
                        diag(&format!("gui-server: accept failed: {e}"));
                        continue;
                    },
                };
                let state = state.clone();
                let tx = tx.clone();
                let csrf_token = csrf_clone.clone();
                let log = log_clone.clone();
                std::thread::spawn(move || {
                    let ctx = ConnectionContext {
                        state: &state,
                        tx: &tx,
                        csrf_token: &csrf_token,
                        log: &log,
                    };
                    if let Err(e) = handle_connection(stream, &ctx) {
                        diag(&format!("gui-server: connection: {e}"));
                    }
                });
            }
        });

        Ok(Server {
            port,
            csrf_token,
            log,
        })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?t={}", self.port, self.csrf_token)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn log(&self) -> &ActivityLog {
        &self.log
    }
}
