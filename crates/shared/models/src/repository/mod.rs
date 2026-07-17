//! Repository lifecycle traits and query value objects.
//!
//! [`ServiceLifecycle`] and [`ServiceRecord`] model managed-service
//! state, [`WhereClause`] composes filter predicates, and
//! [`process_utils`] filters records by running-process status.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod process_utils;
pub mod query_builder;
pub mod service;

pub use process_utils::filter_running_services;
pub use query_builder::WhereClause;
pub use service::{ServiceLifecycle, ServiceRecord};
