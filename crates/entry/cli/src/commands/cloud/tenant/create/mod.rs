mod cloud;
mod local;

pub use cloud::{create_cloud_tenant, swap_to_external_host};
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
        "systemprompt".to_string()
    } else if sanitized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("db_{}", sanitized)
    } else {
        sanitized
    }
}
