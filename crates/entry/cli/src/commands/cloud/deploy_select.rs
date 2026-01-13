use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
use systemprompt_cloud::{get_cloud_paths, CloudPath, ProfilePath, ProjectContext, TenantStore};
use systemprompt_core_logging::CliService;
use systemprompt_loader::ProfileLoader;
use systemprompt_models::Profile;

pub struct DiscoveredProfile {
    pub name: String,
    pub path: PathBuf,
    pub profile: Profile,
}

pub struct DeployableProfile {
    pub name: String,
    pub path: PathBuf,
    pub profile: Profile,
    pub tenant_name: Option<String>,
    pub hostname: Option<String>,
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

pub fn discover_deployable_profiles(tenant_store: &TenantStore) -> Result<Vec<DeployableProfile>> {
    let profiles = discover_profiles()?;

    let deployable = profiles
        .into_iter()
        .filter_map(|p| to_deployable_profile(p, tenant_store))
        .collect();

    Ok(deployable)
}

fn to_deployable_profile(
    discovered: DiscoveredProfile,
    tenant_store: &TenantStore,
) -> Option<DeployableProfile> {
    if discovered.profile.target != systemprompt_models::ProfileType::Cloud {
        return None;
    }

    let cloud = discovered.profile.cloud.as_ref()?;
    let tenant_id = cloud.tenant_id.as_ref()?;
    let tenant = tenant_store.find_tenant(tenant_id);

    Some(DeployableProfile {
        name: discovered.name,
        path: discovered.path,
        profile: discovered.profile,
        tenant_name: tenant.map(|t| t.name.clone()),
        hostname: tenant.and_then(|t| t.hostname.clone()),
    })
}

pub fn select_profile_interactive(profiles: &[DeployableProfile]) -> Result<usize> {
    let options: Vec<String> = profiles
        .iter()
        .map(|p| {
            let target = p.hostname.as_deref().unwrap_or("unknown");
            let tenant = p.tenant_name.as_deref().unwrap_or("unknown");
            format!("{} → {} ({})", p.name, tenant, target)
        })
        .collect();

    Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select profile to deploy")
        .items(&options)
        .default(0)
        .interact()
        .context("Failed to select profile")
}

pub fn resolve_profile(profile_name: Option<&str>) -> Result<(Profile, PathBuf)> {
    if let Some(name) = profile_name {
        return resolve_profile_by_name(name);
    }

    resolve_profile_interactive()
}

fn resolve_profile_by_name(name: &str) -> Result<(Profile, PathBuf)> {
    let ctx = ProjectContext::discover();
    let profile_path = ctx.profile_path(name, ProfilePath::Config);

    if !profile_path.exists() {
        bail!("Profile '{}' not found at {}", name, profile_path.display());
    }

    let profile = ProfileLoader::load_from_path(&profile_path)
        .with_context(|| format!("Failed to load profile: {}", name))?;

    Ok((profile, profile_path))
}

fn resolve_profile_interactive() -> Result<(Profile, PathBuf)> {
    let cloud_paths = get_cloud_paths()?;
    let tenants_path = cloud_paths.resolve(CloudPath::Tenants);
    let tenant_store = TenantStore::load_from_path(&tenants_path).unwrap_or_default();

    let profiles = discover_deployable_profiles(&tenant_store)?;

    if profiles.is_empty() {
        bail!(
            "No deployable profiles found.\nCreate a cloud profile with: systemprompt cloud \
             profile create <name>"
        );
    }

    CliService::section("Select Profile");
    let selection = select_profile_interactive(&profiles)?;
    let selected = &profiles[selection];

    Ok((selected.profile.clone(), selected.path.clone()))
}
