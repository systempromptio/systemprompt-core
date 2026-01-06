use anyhow::{Context, Result};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::path::Path;
use systemprompt_models::Profile;

pub fn generate_display_name(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "dev" | "development" => "Development".to_string(),
        "prod" | "production" => "Production".to_string(),
        "staging" | "stage" => "Staging".to_string(),
        "test" | "testing" => "Test".to_string(),
        "local" => "Local Development".to_string(),
        "cloud" => "Cloud".to_string(),
        _ => capitalize_first(name),
    }
}

fn capitalize_first(name: &str) -> String {
    let mut chars = name.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

pub fn generate_jwt_secret() -> String {
    let mut rng = thread_rng();
    (0..64)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect()
}

pub fn save_profile_yaml(profile: &Profile, path: &Path, header: Option<&str>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    let yaml = serde_yaml::to_string(profile).context("Failed to serialize profile")?;

    let content = header.map_or_else(|| yaml.clone(), |h| format!("{}\n\n{}", h, yaml));

    std::fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))?;

    Ok(())
}
