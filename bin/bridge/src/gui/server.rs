use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc::Sender;

pub use crate::activity::{ActivityLog, LogEntry, activity_log};
use crate::gui::connection::{ConnectionContext, handle_connection};
use crate::gui::events::UiEvent;
use crate::gui::server_util::mint_csrf_token;
use crate::gui::state::AppState;
use crate::obs::output::diag;

#[derive(Clone)]
pub struct Server {
    port: u16,
    csrf_token: String,
}

impl Server {
    #[tracing::instrument(skip(state, tx))]
    pub fn start(state: Arc<AppState>, tx: Sender<UiEvent>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let csrf_token = mint_csrf_token();
        let log: &'static ActivityLog = activity_log();
        tracing::info!(port, "gui-server listening");
        crate::single_instance::write_running_port(port, &csrf_token);

        let csrf_clone = csrf_token.clone();
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
                std::thread::spawn(move || {
                    let ctx = ConnectionContext {
                        state: &state,
                        tx: &tx,
                        csrf_token: &csrf_token,
                        log,
                    };
                    if let Err(e) = handle_connection(stream, &ctx) {
                        diag(&format!("gui-server: connection: {e}"));
                    }
                });
            }
        });

        Ok(Server { port, csrf_token })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/?t={}", self.port, self.csrf_token)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn log(&self) -> &ActivityLog {
        activity_log()
    }
}
