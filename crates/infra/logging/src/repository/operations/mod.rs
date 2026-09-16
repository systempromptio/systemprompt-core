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
    cleanup_logs_before, clear_all_logs, count_logs_before, count_logs_for_users, create_log,
    delete_log, delete_logs_for_users, delete_logs_multiple, distinct_log_user_ids, update_log,
};
pub(super) use queries::{get_log, list_logs, list_logs_by_module_patterns, list_logs_paginated};
