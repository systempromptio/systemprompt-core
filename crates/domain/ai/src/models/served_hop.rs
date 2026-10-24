//! The catalog hop an AI request was served by, as the audit row records it.
//!
//! `provider` stays the vendor account name the reports have always grouped
//! by; `credential` and `endpoint` are the catalog ids that billed and carried
//! the request (`services/ai/credentials.yaml`, `provider/endpoint`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServedHop<'a> {
    pub provider: &'a str,
    pub credential: &'a str,
    pub endpoint: &'a str,
}

/// The credential and endpoint of the hop an audit row opens on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogHop {
    pub credential: String,
    pub endpoint: String,
}
