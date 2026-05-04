//! Client-Initiated Metadata Discovery (CIMD) fetcher and validator.

mod fetcher;
mod validator;

pub use fetcher::CimdFetcher;
pub use validator::ClientValidator;
