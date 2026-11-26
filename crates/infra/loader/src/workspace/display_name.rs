//! The display name a new profile is given from its name.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[must_use]
pub fn generate_display_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "dev" | "development" => "Development".to_owned(),
        "prod" | "production" => "Production".to_owned(),
        "staging" | "stage" => "Staging".to_owned(),
        "test" | "testing" => "Test".to_owned(),
        "local" => "Local Development".to_owned(),
        "cloud" => "Cloud".to_owned(),
        _ => capitalize_first(name),
    }
}

fn capitalize_first(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}
