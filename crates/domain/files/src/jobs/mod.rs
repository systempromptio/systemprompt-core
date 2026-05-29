//! Scheduled jobs for the files domain.
//!
//! Exposes [`FileIngestionJob`], which scans the configured storage directory
//! and registers discovered image files in the database.

mod file_ingestion;

pub use file_ingestion::FileIngestionJob;
