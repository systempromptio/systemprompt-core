//! Google service-account credentials, re-exported from the security crate.
//!
//! The implementation moved to [`systemprompt_security::google`] so that boot
//! (Vertex model discovery) and request time (this gateway) mint tokens from
//! one cache. This module stays as the gateway's name for it: every call site
//! and test that already reads `credentials::google::…` keeps reading it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_security::google::{ServiceAccountKey, access_token};
