//! The part of a cloud profile a deploy or backup reads on the operator's
//! host.
//!
//! A deploy ships a profile without running it, so it loads the management
//! view ([`ManagedProfile`]): variables that exist only on the target stay
//! unresolved and never fail the deploy unless a section the deploy reads
//! holds one. The Vault bootstrap variables are discovered from the profile
//! as written, so a placeholder is pushed by name whether or not this shell
//! sets it. A profile with no unresolved value is also loaded whole, so it
//! is validated exactly as it is at boot.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use systemprompt_cloud::ProfilePath;
use systemprompt_identifiers::ProfileName;
use systemprompt_manifest::ProfileType;
use systemprompt_manifest::profile::{
    CloudConfig, HostingProvider, LicenceConfig, ManagedProfile, SecretsConfig,
};

use super::pipeline::DeploySecretsSource;

/// The deploy-relevant sections of a cloud profile, read without its
/// target-only variables.
#[derive(Debug, Clone)]
pub struct DeployProfile {
    pub path: PathBuf,
    pub name: ProfileName,
    pub target: ProfileType,
    pub cloud: Option<CloudConfig>,
    pub secrets: DeploySecretsSource,
    pub signing_key_path: PathBuf,
    pub licence_secret_required: bool,
    pub managed: ManagedProfile,
}

impl DeployProfile {
    pub fn load(profile_path: &Path) -> Result<Self> {
        let managed = ManagedProfile::load(profile_path)
            .with_context(|| format!("Failed to load profile: {}", profile_path.display()))?;
        Self::from_managed(managed)
    }

    pub fn from_managed(managed: ManagedProfile) -> Result<Self> {
        let path = managed.path().to_path_buf();
        if managed.unresolved().is_empty() {
            managed
                .profile()
                .with_context(|| format!("Failed to load profile: {}", path.display()))?;
        }
        let profile_dir = path
            .parent()
            .ok_or_else(|| anyhow!("Invalid profile path: {}", path.display()))?
            .to_path_buf();
        let name: String = managed
            .section("name")?
            .ok_or_else(|| anyhow!("Profile {} has no `name`", path.display()))?;
        let name = ProfileName::try_new(name.as_str())
            .with_context(|| format!("Profile name '{name}' is not a valid profile name"))?;
        let target = managed
            .section::<ProfileType>("target")?
            .unwrap_or_default();
        let cloud = managed.section::<CloudConfig>("cloud")?;
        let raw_secrets = managed.raw_section::<SecretsConfig>("secrets")?;
        let secrets = DeploySecretsSource::from_profile(
            raw_secrets.as_ref(),
            ProfilePath::Secrets.resolve(&profile_dir),
        );
        let configured_key = managed
            .section::<PathBuf>("security.signing_key_path")?
            .unwrap_or_else(|| {
                PathBuf::from(systemprompt_manifest::profile::DEFAULT_SIGNING_KEY_PATH)
            });
        let signing_key_path = if configured_key.is_absolute() {
            configured_key
        } else {
            profile_dir.join(configured_key)
        };
        let licence_secret_required = managed
            .section::<LicenceConfig>("licence")?
            .unwrap_or_default()
            .enabled();
        Ok(Self {
            path,
            name,
            target,
            cloud,
            secrets,
            signing_key_path,
            licence_secret_required,
            managed,
        })
    }

    #[must_use]
    pub fn hosting_provider(&self) -> HostingProvider {
        self.cloud
            .as_ref()
            .map_or_else(HostingProvider::default, |cloud| cloud.provider)
    }

    #[must_use]
    pub fn profile_dir(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }
}
