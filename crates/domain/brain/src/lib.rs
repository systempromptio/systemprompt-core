//! Brain: the organisation's revisioned, scoped memory.
//!
//! Producers append a [`BrainEvent`] to the durable outbox inside their own
//! transaction through the injected [`DynBrainSink`]
//! ([`sink::OutboxBrainSink`]). The capture consumer turns events into
//! facts; this crate holds the store those facts live in.
//!
//! The store is a set of `brain_*` tables ([`schema::BRAIN_SCHEMAS`],
//! registered by [`extension::BrainExtension`]) behind the repositories in
//! [`repository::BrainRepositories`]: declared plane rows (kinds, topics,
//! interests, workflow profiles, config documents), subjects and trigram
//! aliases, facts with their append-only revisions, the capture ledgers
//! (extractions, per-event results, failures, disputes, tombstones), and
//! the read side's recalls, outcomes and proposals. A stored fact changes
//! only through [`repository::FactRepository::apply`], which writes the
//! revision row on the same connection. [`version`] names the pipeline
//! version a fact was built under.
//!
//! The crate is built behind the facade's `brain` feature (in `full`, not
//! in `core`). Its schema requires the `pg_trgm` Postgres extension, which
//! the schema creates if absent.
//!
//! Errors are [`error::BrainError`]; repositories answer
//! `systemprompt_traits::RepositoryError`.
//!
//! [`BrainEvent`]: systemprompt_models::brain::BrainEvent
//! [`DynBrainSink`]: systemprompt_extension::brain::DynBrainSink
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod error;
pub mod extension;
pub mod models;
pub mod plane;
pub mod repository;
pub mod schema;
pub mod sink;
pub mod version;

pub use error::{BrainError, BrainResult};
pub use extension::BrainExtension;
pub use models::{FactChange, FactFilter, Inserted, NewFact, NewRecall};
pub use repository::{BrainRepositories, FactRepository};
pub use sink::OutboxBrainSink;
pub use version::{pipeline_inputs, pipeline_version};
