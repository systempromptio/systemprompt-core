//! Tenant creation flows for the `cloud tenant create` command.
//!
//! Routes to [`create_local_tenant`] for a tenant-owned Docker container or
//! [`create_external_tenant`] for a user-supplied database.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod local;

pub use local::{create_external_tenant, create_local_tenant};

fn sanitize_database_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        "systemprompt".to_owned()
    } else if sanitized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("db_{}", sanitized)
    } else {
        sanitized
    }
}
