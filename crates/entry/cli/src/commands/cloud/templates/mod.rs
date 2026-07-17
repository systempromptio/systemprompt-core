//! Static HTML pages served by the local callback server during cloud flows.
//!
//! Re-exports the checkout pages (waiting, success, error) and the OAuth
//! login pages (success, error) rendered in the user's browser while the CLI
//! waits on a redirect from the cloud backend.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod checkout;
mod oauth;

pub use checkout::{
    ERROR_HTML as CHECKOUT_ERROR_HTML, SUCCESS_HTML as CHECKOUT_SUCCESS_HTML, WAITING_HTML,
};
pub use oauth::{ERROR_HTML as AUTH_ERROR_HTML, SUCCESS_HTML as AUTH_SUCCESS_HTML};
