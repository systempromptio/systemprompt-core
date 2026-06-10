//! Tenant creation flows for the `cloud tenant create` command.
//!
//! Routes to the cloud-subscription flow ([`create_cloud_tenant`]) or one of
//! the local flows ([`create_local_tenant`] for a managed Docker container,
//! [`create_external_tenant`] for a user-supplied database).

mod cloud;
mod local;
mod progress;

pub use cloud::create_cloud_tenant;
pub use local::{create_external_tenant, create_local_tenant};
pub use systemprompt_cloud::tenants::swap_to_external_host;

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
