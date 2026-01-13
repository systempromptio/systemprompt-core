use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_loader::ProfileLoader;
use systemprompt_models::Profile;

#[derive(Debug)]
pub struct DiscoveredProfile {
    pub name: String,
    pub path: PathBuf,
    pub profile: Profile,
}

pub fn discover_profiles() -> Result<Vec<DiscoveredProfile>> {
    let ctx = ProjectContext::discover();
    let profiles_dir = ctx.profiles_dir();

    if !profiles_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(&profiles_dir).with_context(|| {
        format!(
            "Failed to read profiles directory: {}",
            profiles_dir.display()
        )
    })?;

    let profiles = entries
        .filter_map(std::result::Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| build_discovered_profile(&e))
        .collect();

    Ok(profiles)
}

fn build_discovered_profile(entry: &std::fs::DirEntry) -> Option<DiscoveredProfile> {
    let profile_yaml = ProfilePath::Config.resolve(&entry.path());
    if !profile_yaml.exists() {
        return None;
    }

    let name = entry.file_name().to_string_lossy().to_string();
    let profile = ProfileLoader::load_from_path(&profile_yaml).ok()?;

    Some(DiscoveredProfile {
        name,
        path: profile_yaml,
        profile,
    })
}

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
    let mut rng = rng();
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
