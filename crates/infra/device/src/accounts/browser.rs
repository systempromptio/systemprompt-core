//! Opening a page in the user's browser, injected so tests can stand in for it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;

use url::Url;

/// Opens URLs in the user's default browser. The shell implements it with the
/// OS; tests record or answer the call. `open` returns once the OS accepted the
/// request.
pub trait Browser: Send + Sync + fmt::Debug {
    fn open(&self, url: &Url) -> Result<(), BrowserError>;
}

/// The browser could not be asked to open a page.
#[derive(Debug, thiserror::Error)]
#[error("the browser could not be opened")]
pub struct BrowserError(#[source] pub std::io::Error);
