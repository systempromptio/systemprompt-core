//! The one-shot loopback listener the browser returns to after the device-link
//! page.
//!
//! The device-link page drops any `state` of its own but keeps the redirect's
//! query verbatim, so the listener puts a random `state` in the redirect URL
//! and accepts only a `GET /callback` that carries it back with a `code` or
//! `error=denied`. Anything else is answered `400` and the listener keeps
//! waiting. It binds `127.0.0.1` (not `localhost`, which may resolve to `::1`,
//! a redirect the page refuses) on an ephemeral port, gives up after ten
//! minutes, and stops when cancelled. Each connection is served by an owned
//! task aborted when the wait ends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod page;
mod request;

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::protocol::OneTimeCode;
use systemprompt_traits::OwnedTask;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

use self::page::{Page, render, respond};
use self::request::{CALLBACK_PATH, Verdict, judge, read_request_line};

pub const CALLBACK_EXPIRY: Duration = Duration::from_secs(600);

const REQUEST_READ_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECTION_TASK: &str = "sp-client.loopback.connection";

/// The listener could not be set up or stopped accepting.
#[derive(Debug, thiserror::Error)]
pub enum LoopbackError {
    #[error("the loopback listener could not bind 127.0.0.1")]
    Bind(#[source] std::io::Error),
    #[error("the loopback listener stopped accepting connections")]
    Accept(#[source] std::io::Error),
    #[error("the loopback redirect URL is not valid")]
    Url(#[source] url::ParseError),
}

/// How the browser round trip ended.
#[derive(Debug)]
pub enum CallbackOutcome {
    Code(OneTimeCode),
    Denied,
    Expired,
    Cancelled,
}

/// A bound loopback listener waiting for exactly one sign-in callback.
pub struct LoopbackCallback {
    listener: TcpListener,
    redirect: Url,
    state: Arc<str>,
    product: Arc<str>,
    cancel: CancellationToken,
    expiry: Duration,
}

impl fmt::Debug for LoopbackCallback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoopbackCallback")
            .field("port", &self.redirect.port())
            .field("expiry", &self.expiry)
            .finish_non_exhaustive()
    }
}

impl LoopbackCallback {
    pub async fn bind(product: &str) -> Result<Self, LoopbackError> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
            .await
            .map_err(LoopbackError::Bind)?;
        let port = listener.local_addr().map_err(LoopbackError::Bind)?.port();
        let state = random_state();
        let redirect = Url::parse(&format!(
            "http://127.0.0.1:{port}{CALLBACK_PATH}?state={state}"
        ))
        .map_err(LoopbackError::Url)?;
        tracing::debug!(port, "loopback sign-in listener bound");
        Ok(Self {
            listener,
            redirect,
            state: Arc::from(state),
            product: Arc::from(product),
            cancel: CancellationToken::new(),
            expiry: CALLBACK_EXPIRY,
        })
    }

    #[must_use]
    pub const fn with_expiry(mut self, expiry: Duration) -> Self {
        self.expiry = expiry;
        self
    }

    #[must_use]
    pub const fn redirect_url(&self) -> &Url {
        &self.redirect
    }

    #[must_use]
    pub fn cancellation(&self) -> CancellationToken {
        self.cancel.clone()
    }

    pub async fn wait(self) -> Result<CallbackOutcome, LoopbackError> {
        let Self {
            listener,
            state,
            product,
            cancel,
            expiry,
            ..
        } = self;
        let deadline = tokio::time::sleep(expiry);
        tokio::pin!(deadline);
        let (sender, mut receiver) = mpsc::channel::<CallbackOutcome>(1);
        let shared = Arc::new(Shared {
            state,
            product,
            claimed: AtomicBool::new(false),
            outcome: sender,
        });
        let mut connections: Vec<OwnedTask<()>> = Vec::new();
        loop {
            tokio::select! {
                () = cancel.cancelled() => return Ok(CallbackOutcome::Cancelled),
                () = &mut deadline => return Ok(CallbackOutcome::Expired),
                Some(outcome) = receiver.recv() => return Ok(outcome),
                accepted = listener.accept() => {
                    let (stream, _peer) = accepted.map_err(LoopbackError::Accept)?;
                    connections.retain(|connection| !connection.is_finished());
                    connections.push(OwnedTask::spawn(CONNECTION_TASK, serve(stream, Arc::clone(&shared))));
                },
            }
        }
    }
}

#[derive(Debug)]
struct Shared {
    state: Arc<str>,
    product: Arc<str>,
    claimed: AtomicBool,
    outcome: mpsc::Sender<CallbackOutcome>,
}

impl Shared {
    fn claim(&self) -> bool {
        self.claimed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

async fn serve(mut stream: TcpStream, shared: Arc<Shared>) {
    let Ok(Some(line)) =
        tokio::time::timeout(REQUEST_READ_TIMEOUT, read_request_line(&mut stream)).await
    else {
        return;
    };
    let (status, page, result) = match judge(&line, &shared.state) {
        Verdict::Code(code) if shared.claim() => {
            ("200 OK", Page::SignedIn, Some(CallbackOutcome::Code(code)))
        },
        Verdict::Denied if shared.claim() => {
            ("200 OK", Page::Denied, Some(CallbackOutcome::Denied))
        },
        Verdict::Code(_) | Verdict::Denied | Verdict::Invalid => {
            ("400 Bad Request", Page::Invalid, None)
        },
    };
    if let Err(error) = respond(&mut stream, status, &render(page, &shared.product)).await {
        tracing::debug!(error = %error, "loopback answer not delivered");
    }
    if let Some(result) = result
        && shared.outcome.send(result).await.is_err()
    {
        tracing::debug!("loopback callback arrived after the listener stopped");
    }
}

fn random_state() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}
