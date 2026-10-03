//! Typed failures of a caller-supplied image fetch.
//!
//! [`ImageFetchFault`] says what went wrong and whose fault it was: a URL,
//! host, redirect or body the caller chose is a client error, while a timeout,
//! an unbuildable client or a broken read is the gateway's. The rejection a
//! guarded client raised deep inside a `reqwest` error is copied into the owned
//! [`GuardedRejection`] so the fault keeps the `reqwest` error as its source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;
use std::time::Duration;

use systemprompt_client::GuardedConnectError;
use systemprompt_models::net::OutboundUrlError;

#[derive(Debug, thiserror::Error)]
#[error("image url {url} could not be inlined: {fault}")]
pub struct ImageFetchFailed {
    pub url: String,
    #[source]
    pub fault: ImageFetchFault,
}

impl ImageFetchFailed {
    #[must_use]
    pub fn caller_fault(&self) -> bool {
        self.fault.caller_fault()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ImageFetchFault {
    #[error("url rejected: {0}")]
    Url(#[source] OutboundUrlError),
    #[error("missing host")]
    MissingHost,
    #[error("{host} resolves to blocked address {addr}")]
    BlockedAddress { host: String, addr: IpAddr },
    #[error("cannot build guarded image client")]
    Client(#[source] reqwest::Error),
    #[error("fetch exceeded {0:?}")]
    Timeout(Duration),
    #[error("{rejection}")]
    Guarded {
        rejection: GuardedRejection,
        #[source]
        source: reqwest::Error,
    },
    #[error("request failed")]
    Request(#[source] reqwest::Error),
    #[error("host returned {0}")]
    HostStatus(reqwest::StatusCode),
    #[error("no content-type")]
    NoContentType,
    #[error("content-type {0} is not an inlineable image")]
    UnsupportedType(String),
    #[error("larger than {0} bytes")]
    TooLarge(usize),
    #[error("empty response body")]
    EmptyBody,
    #[error("read failed")]
    Read(#[source] reqwest::Error),
}

impl ImageFetchFault {
    #[must_use]
    pub fn caller_fault(&self) -> bool {
        match self {
            Self::Url(_)
            | Self::MissingHost
            | Self::BlockedAddress { .. }
            | Self::Guarded { .. }
            | Self::HostStatus(_)
            | Self::NoContentType
            | Self::UnsupportedType(_)
            | Self::TooLarge(_)
            | Self::EmptyBody => true,
            Self::Request(source) => source.is_redirect(),
            Self::Client(_) | Self::Timeout(_) | Self::Read(_) => false,
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum GuardedRejection {
    #[error("redirect rejected: redirect to {url} refused")]
    RedirectRefused { url: String },
    #[error("redirect rejected: more than {0} redirects")]
    TooManyRedirects(usize),
    #[error("host rejected: cannot resolve {0}")]
    Unresolvable(String),
    #[error("host rejected: host {host} resolves to blocked address {addr}")]
    BlockedAddress { host: String, addr: IpAddr },
}

impl From<&GuardedConnectError> for GuardedRejection {
    fn from(error: &GuardedConnectError) -> Self {
        match error {
            GuardedConnectError::RedirectRefused { url, .. } => {
                Self::RedirectRefused { url: url.clone() }
            },
            GuardedConnectError::TooManyRedirects(limit) => Self::TooManyRedirects(*limit),
            GuardedConnectError::Unresolvable { host, .. } => Self::Unresolvable(host.clone()),
            GuardedConnectError::BlockedAddress { host, addr } => Self::BlockedAddress {
                host: host.clone(),
                addr: *addr,
            },
        }
    }
}
