//! SQL operations backing the log repository.
//!
//! Splits read paths ([`queries`]) from write paths ([`mutations`]) over the
//! `logs` table and re-exports the crate-internal entry points the repository
//! facade composes (fetch, list, paginate, create, update, delete, retention
//! cleanup).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod mutations;
mod queries;

pub(super) use mutations::{
    cleanup_logs_before, clear_all_logs, count_logs_before, create_log, delete_log,
    delete_logs_multiple, update_log,
};
pub(super) use queries::{get_log, list_logs, list_logs_by_module_patterns, list_logs_paginated};
