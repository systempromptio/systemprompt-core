use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::mpsc::Sender;

pub use crate::activity::{ActivityLog, LogEntry, activity_log};
use crate::gui::events::UiEvent;
use crate::gui::server_util::{constant_time_eq, mint_csrf_token};
use crate::gui::state::AppState;
use crate::obs::output::diag;

#[derive(Clone)]
pub struct Server {
    port: u16,
    #[allow(dead_code)]
    csrf_token: String,
}

impl Server {
    #[tracing::instrument(skip(_state, tx))]
    pub fn start(_state: Arc<AppState>, tx: Sender<UiEvent>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let csrf_token = mint_csrf_token();
        tracing::info!(port, "single-instance focus server listening");
        crate::single_instance::write_running_port(port, &csrf_token);

        let csrf_clone = csrf_token.clone();
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let stream = match conn {
                    Ok(s) => s,
                    Err(e) => {
                        diag(&format!("focus-server: accept failed: {e}"));
                        continue;
                    },
                };
                let tx = tx.clone();
                let token = csrf_clone.clone();
                std::thread::spawn(move || handle_focus(stream, &tx, &token));
            }
        });

        Ok(Server { port, csrf_token })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn log(&self) -> &'static ActivityLog {
        activity_log()
    }
}

fn handle_focus(mut stream: std::net::TcpStream, tx: &Sender<UiEvent>, csrf: &str) {
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let path_with_query = request_line.split_whitespace().nth(1).unwrap_or("");
    let (path, query) = path_with_query
        .split_once('?')
        .unwrap_or((path_with_query, ""));
    let supplied_token = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("t="))
        .unwrap_or("");
    if !request_line.starts_with("POST ")
        || path != "/api/focus_window"
        || !constant_time_eq(supplied_token.as_bytes(), csrf.as_bytes())
    {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        return;
    }
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() {
            return;
        }
        if header == "\r\n" || header.is_empty() {
            break;
        }
    }
    let _ = tx.send(UiEvent::FocusWindow);
    let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n");
}
