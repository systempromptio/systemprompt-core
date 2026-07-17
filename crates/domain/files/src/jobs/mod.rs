//! Scheduled jobs for the files domain.
//!
//! Exposes [`FileIngestionJob`], which scans the configured storage directory
//! and registers discovered image files in the database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod file_ingestion;

pub use file_ingestion::FileIngestionJob;
