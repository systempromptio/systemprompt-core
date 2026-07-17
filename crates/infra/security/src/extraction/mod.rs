//! Token extraction from inbound HTTP requests and id-header injection.
//!
//! Three extractors cover the three transport contracts the API supports:
//! the `Authorization` bearer header, the MCP proxy header, and the
//! browser cookie. The [`HeaderInjector`] runs in the opposite direction
//! to stamp typed identifiers onto outbound requests.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod cookie;
mod header;
mod token;

pub use cookie::{CookieExtractionError, CookieExtractor};
pub use header::{HeaderExtractor, HeaderInjectionError, HeaderInjector};
pub use token::{ExtractionMethod, TokenExtractionError, TokenExtractor};
