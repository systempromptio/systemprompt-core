//! Published-content value types.
//!
//! [`ContentLink`] describes an inter-page link and [`IngestionReport`]
//! summarises the outcome of a content ingestion pass.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ingestion;
mod link;

pub use ingestion::IngestionReport;
pub use link::ContentLink;
