use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rand::distr::Alphanumeric;
use rand::{rng, Rng};
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_loader::ProfileLoader;
use systemprompt_models::Profile;

#[derive(Debug, thiserror::Error)]
pub enum ProfileResolutionError {
    #[error(
        "No profiles found.\n\nCreate a profile with: systemprompt cloud profile create <name>"
    )]
    NoProfilesFound,

    #[error(
        "Profile '{0}' not found.\n\nRun 'systemprompt cloud profile list' to see available \
         profiles."
    )]
    ProfileNotFound(String),

    #[error("Profile discovery failed: {0}")]
    DiscoveryFailed(#[from] anyhow::Error),

    #[error(
        "Multiple profiles found: {profiles:?}\n\nUse --profile <name> or 'systemprompt admin \
         session switch <profile>'"
    )]
    MultipleProfilesFound { profiles: Vec<String> },
}

pub fn resolve_profile_path(
    cli_override: Option<&str>,
    from_session: Option<PathBuf>,
) -> Result<PathBuf, ProfileResolutionError> {
    if let Some(profile_input) = cli_override {
        return resolve_profile_input(profile_input);
    }

    if let Ok(path_str) = std::env::var("SYSTEMPROMPT_PROFILE") {
        return resolve_profile_input(&path_str);
    }

    if let Some(path) = from_session.filter(|p| p.exists()) {
        return Ok(path);
    }

    let mut profiles = discover_profiles()?;
    match profiles.len() {
        0 => Err(ProfileResolutionError::NoProfilesFound),
        1 => Ok(profiles.swap_remove(0).path),
        _ => Err(ProfileResolutionError::MultipleProfilesFound {
            profiles: profiles.iter().map(|p| p.name.clone()).collect(),
        }),
    }
}

fn is_path_input(input: &str) -> bool {
    let path = Path::new(input);
    let has_yaml_extension = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"));

    input.contains(std::path::MAIN_SEPARATOR)
        || input.contains('/')
        || has_yaml_extension
        || input.starts_with('.')
        || input.starts_with('~')
}

fn resolve_profile_input(input: &str) -> Result<PathBuf, ProfileResolutionError> {
    if is_path_input(input) {
        return resolve_profile_from_path(input);
    }
    resolve_profile_by_name(input)?
        .ok_or_else(|| ProfileResolutionError::ProfileNotFound(input.to_string()))
}

fn resolve_profile_from_path(path_str: &str) -> Result<PathBuf, ProfileResolutionError> {
    let path = expand_path(path_str);

    if path.exists() {
        return Ok(path);
    }

    let profile_yaml = path.join("profile.yaml");
    if profile_yaml.exists() {
        return Ok(profile_yaml);
    }

    Err(ProfileResolutionError::ProfileNotFound(
        path_str.to_string(),
    ))
}

fn expand_path(path_str: &str) -> PathBuf {
    if path_str.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(
                path_str
                    .strip_prefix("~/")
                    .unwrap_or_else(|| &path_str[1..]),
            );
        }
    }
    PathBuf::from(path_str)
}

pub fn resolve_profile_with_data(
    profile_input: &str,
) -> Result<(PathBuf, Profile), ProfileResolutionError> {
    let path = resolve_profile_input(profile_input)?;
    let profile =
        ProfileLoader::load_from_path(&path).map_err(ProfileResolutionError::DiscoveryFailed)?;
    Ok((path, profile))
}

fn resolve_profile_by_name(name: &str) -> Result<Option<PathBuf>, ProfileResolutionError> {
    let profiles = discover_profiles()?;
    Ok(profiles
        .into_iter()
        .find(|p| p.name == name)
        .map(|p| p.path))
}

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
