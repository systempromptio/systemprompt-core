//! Static HTML pages served by the local callback server during cloud flows.
//!
//! Re-exports the OAuth login pages (success, error) rendered in the user's
//! browser while the CLI waits on a redirect from the cloud backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod oauth;

pub use oauth::{ERROR_HTML as AUTH_ERROR_HTML, SUCCESS_HTML as AUTH_SUCCESS_HTML};
