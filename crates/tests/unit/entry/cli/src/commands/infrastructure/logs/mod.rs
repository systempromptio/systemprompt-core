//! Tests for `infra logs` command output builders. These pin each converged
//! command to the artifact variant it renders to stdout (table for list, card
//! for show/stats/summary/audit) and to a message artifact on the
//! empty/not-found path.

mod logs_builders;
mod request_builders;
