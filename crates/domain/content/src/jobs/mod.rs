//! Scheduled content jobs.
//!
//! Exposes [`execute_content_ingestion`], the job entry point that ingests
//! configured content sources into the content store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content_ingestion;

pub use content_ingestion::execute_content_ingestion;
