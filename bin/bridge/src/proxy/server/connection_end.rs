//! Classification and logging of how a served proxy connection ended.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// How a served proxy connection ended when hyper reports it as an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionEnd {
    PeerGone,
    UpstreamIdle,
    Failed,
}

#[must_use]
pub fn classify_connection_end(e: &hyper::Error) -> ConnectionEnd {
    if e.is_incomplete_message() || e.is_closed() || e.is_canceled() {
        return ConnectionEnd::PeerGone;
    }
    let mut cause = std::error::Error::source(e);
    while let Some(inner) = cause {
        if let Some(io) = inner.downcast_ref::<std::io::Error>() {
            if matches!(
                io.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::UnexpectedEof
            ) {
                return ConnectionEnd::PeerGone;
            }
            if io
                .get_ref()
                .and_then(|wrapped| wrapped.downcast_ref::<reqwest::Error>())
                .is_some_and(reqwest::Error::is_timeout)
            {
                return ConnectionEnd::UpstreamIdle;
            }
        }
        if inner
            .downcast_ref::<reqwest::Error>()
            .is_some_and(reqwest::Error::is_timeout)
        {
            return ConnectionEnd::UpstreamIdle;
        }
        cause = inner.source();
    }
    ConnectionEnd::Failed
}

pub(super) fn log_connection_end(e: &hyper::Error) {
    let cause = std::error::Error::source(e).map(ToString::to_string);
    match classify_connection_end(e) {
        ConnectionEnd::PeerGone => {},
        ConnectionEnd::UpstreamIdle => tracing::debug!(
            target: "systemprompt_bridge::proxy",
            error = %e,
            cause = ?cause,
            "upstream stream went idle past the read limit; the response was ended"
        ),
        ConnectionEnd::Failed => tracing::warn!(
            target: "systemprompt_bridge::proxy",
            error = %e,
            cause = ?cause,
            "proxy connection error"
        ),
    }
}
