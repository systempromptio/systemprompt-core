//! Request helpers every HTTP surface shares: the client address resolved
//! against the profile's trusted proxies ([`client_addr`]), the request base
//! URL echoed in OAuth discovery ([`request_base_url`]) and the one builder
//! for the `access_token` session cookie ([`session_cookie`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod client_addr;
pub mod request_base_url;
pub mod session_cookie;

pub use client_addr::{ClientIp, ClientIpConfigMissing, forwarded_headers_ignored, resolve_client_ip};
pub use request_base_url::{RequestBaseUrl, RequestBaseUrlError, resolve as resolve_request_base_url};
pub use session_cookie::{SameSite, SessionCookie};
