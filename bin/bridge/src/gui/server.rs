//! Loopback TCP server backing the GUI webview (focus wake-up, CSRF-gated).
//!
//! One owned thread accepts and answers connections in turn, each bounded by
//! a read deadline and a request-size cap, so a stalled client cannot hold it
//! for longer than one deadline. Stopping raises the thread's stop signal,
//! wakes the blocking accept with a loopback connect and joins the thread.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use crate::gui::UiEventProxy;
use crate::gui::events::UiEvent;
use crate::gui::server_util::{constant_time_eq, mint_csrf_token};
use crate::stdio::diag;
use crate::tasks::{OwnedThread, OwnedThreadError, StopSignal};

const REQUEST_DEADLINE: Duration = Duration::from_secs(2);
const MAX_REQUEST_HEAD: usize = 8 * 1024;
const WAKE_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug)]
pub struct FocusServer {
    port: u16,
    accept: OwnedThread,
}

impl FocusServer {
    #[tracing::instrument(skip(proxy))]
    pub(crate) fn start(proxy: UiEventProxy) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let csrf_token = mint_csrf_token();
        tracing::info!(port, "single-instance focus server listening");
        crate::single_instance::write_running_port(port, &csrf_token)?;

        let accept = OwnedThread::spawn(
            "bridge-focus-server",
            move |stop| accept_loop(&listener, &stop, &proxy, &csrf_token),
            move || wake(port),
        )?;
        Ok(Self { port, accept })
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    pub(crate) fn stop(self) -> Result<(), OwnedThreadError> {
        self.accept.stop()
    }
}

fn accept_loop(listener: &TcpListener, stop: &StopSignal, proxy: &UiEventProxy, csrf: &str) {
    for conn in listener.incoming() {
        if stop.is_raised() {
            return;
        }
        match conn {
            Ok(stream) => {
                if let Err(e) = handle_focus(stream, proxy, csrf) {
                    tracing::error!(error = %e, "focus request failed");
                }
            },
            Err(e) => diag(&format!("focus-server: accept failed: {e}")),
        }
    }
}

fn wake(port: u16) -> std::io::Result<()> {
    TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), WAKE_TIMEOUT)
        .map(drop)
}

fn read_request_head(stream: &mut TcpStream) -> std::io::Result<Option<String>> {
    let deadline = Instant::now() + REQUEST_DEADLINE;
    let mut head = Vec::with_capacity(512);
    let mut chunk = [0u8; 512];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() || head.len() >= MAX_REQUEST_HEAD {
            return Ok(None);
        }
        stream.set_read_timeout(Some(remaining))?;
        match stream.read(&mut chunk) {
            Ok(0) => return Ok(None),
            Ok(n) => head.extend_from_slice(chunk.get(..n).unwrap_or_default()),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                return Ok(None);
            },
            Err(e) if e.kind() == ErrorKind::Interrupted => {},
            Err(e) => return Err(e),
        }
    }
    Ok(Some(String::from_utf8_lossy(&head).into_owned()))
}

fn handle_focus(mut stream: TcpStream, proxy: &UiEventProxy, csrf: &str) -> std::io::Result<()> {
    stream.set_write_timeout(Some(REQUEST_DEADLINE))?;
    let Some(head) = read_request_head(&mut stream)? else {
        return Ok(());
    };
    let request_line = head.lines().next().unwrap_or("");
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
        stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")?;
        return Ok(());
    }
    proxy.send_event(UiEvent::FocusWindow);
    stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")?;
    Ok(())
}
