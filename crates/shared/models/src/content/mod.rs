//! Published-content value types.
//!
//! [`ContentLink`] describes an inter-page link and [`IngestionReport`]
//! summarises the outcome of a content ingestion pass.

mod ingestion;
mod link;

pub use ingestion::IngestionReport;
pub use link::ContentLink;
