//! Persistence layer for the files domain.
//!
//! Exposes [`FileRepository`], the SQLx-backed entry point for file rows and
//! their content associations, together with its [`InsertFileRequest`] builder
//! and [`FileStats`] aggregate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod ai;
mod content;
mod file;

pub use file::{FileRepository, FileStats, InsertFileRequest};
