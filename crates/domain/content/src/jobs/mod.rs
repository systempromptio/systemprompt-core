//! Scheduled content jobs.
//!
//! Exposes [`execute_content_ingestion`], the job entry point that ingests
//! configured content sources into the content store.

mod content_ingestion;

pub use content_ingestion::execute_content_ingestion;
