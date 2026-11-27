//! Headers for remote servers, resolved again before every request so a
//! rotating credential is always current.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use parking_lot::RwLock;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::mcp::error::{BoxError, McpError};

/// Supplies the headers for a remote server.
///
/// It is called before every HTTP request, so it should
/// be cheap; an error fails that request. [`RemoteHeaders`] holds it as `dyn
/// HeaderSource`, which is why the trait uses `async_trait`.
#[async_trait::async_trait]
pub trait HeaderSource: Send + Sync {
    async fn headers(&self) -> Result<Vec<(String, String)>, BoxError>;
}

/// The headers sent to a remote server: a fixed list or a [`HeaderSource`].
/// `Debug` shows header names only.
#[derive(Clone)]
pub enum RemoteHeaders {
    Static(Vec<(String, String)>),
    Source(Arc<dyn HeaderSource>),
}

impl Default for RemoteHeaders {
    fn default() -> Self {
        Self::Static(Vec::new())
    }
}

impl fmt::Debug for RemoteHeaders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(pairs) => {
                let names: Vec<&str> = pairs.iter().map(|(name, _)| name.as_str()).collect();
                f.debug_tuple("Static").field(&names).finish()
            },
            Self::Source(_) => f.write_str("Source(..)"),
        }
    }
}

impl RemoteHeaders {
    pub async fn resolve(&self) -> Result<HeaderMap, McpError> {
        match self {
            Self::Static(pairs) => to_header_map(pairs),
            Self::Source(source) => {
                let pairs = source.headers().await.map_err(McpError::HeaderSource)?;
                to_header_map(&pairs)
            },
        }
    }

    pub(crate) fn same_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Static(a), Self::Static(b)) => a == b,
            (Self::Source(a), Self::Source(b)) => std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b)),
            _ => false,
        }
    }
}

pub(crate) type HeaderCell = Arc<RwLock<RemoteHeaders>>;

pub(crate) fn cell(headers: RemoteHeaders) -> HeaderCell {
    Arc::new(RwLock::new(headers))
}

pub(crate) async fn current(cell: &HeaderCell) -> Result<HeaderMap, McpError> {
    let headers = RemoteHeaders::clone(&cell.read());
    headers.resolve().await
}

fn to_header_map(pairs: &[(String, String)]) -> Result<HeaderMap, McpError> {
    let mut map = HeaderMap::with_capacity(pairs.len());
    for (name, value) in pairs {
        let header_name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|error| invalid(name, &error))?;
        let mut header_value =
            HeaderValue::from_str(value).map_err(|error| invalid(name, &error))?;
        header_value.set_sensitive(true);
        map.insert(header_name, header_value);
    }
    Ok(map)
}

fn invalid(name: &str, error: &dyn std::error::Error) -> McpError {
    tracing::debug!(header = %name, error = %error, "rejected an invalid header");
    McpError::Header {
        name: name.to_owned(),
    }
}
