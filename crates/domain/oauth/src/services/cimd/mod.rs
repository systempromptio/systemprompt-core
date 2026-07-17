//! Client-Initiated Metadata Discovery (CIMD) fetcher and validator.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod fetcher;
mod validator;

pub use fetcher::CimdFetcher;
pub use validator::ClientValidator;
