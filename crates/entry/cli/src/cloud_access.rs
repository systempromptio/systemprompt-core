//! The kernel's one gate onto the cloud member's [`CloudAccess`].
//!
//! The composition hands the CLI a [`DynCloudAccess`] only when it is built
//! with the facade feature `cloud`; a profile that disables the `cloud`
//! member withholds it too. [`require_cloud`] turns either absence into the
//! typed [`CloudAccessError`] the caller returns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::cli::{CloudAccess, CloudAccessError, DynCloudAccess};
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::Profile;

pub const CLOUD_MEMBER: &str = "cloud";

/// The cloud access a build holds and the profile whose `cloud` member
/// switch it answers to.
#[derive(Clone, Copy, Default)]
pub struct CloudScope<'a> {
    pub cloud: Option<&'a DynCloudAccess>,
    pub profile: Option<&'a Profile>,
}

impl std::fmt::Debug for CloudScope<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudScope")
            .field("cloud", &self.cloud.is_some())
            .field("profile", &self.profile.map(|profile| &profile.name))
            .finish()
    }
}

impl CloudScope<'_> {
    pub fn require(&self, action: &'static str) -> Result<&dyn CloudAccess, CloudAccessError> {
        require_cloud(self.cloud, self.profile, action)
    }
}

pub fn require_cloud<'a>(
    cloud: Option<&'a DynCloudAccess>,
    profile: Option<&Profile>,
    action: &'static str,
) -> Result<&'a dyn CloudAccess, CloudAccessError> {
    if let Some(profile) = profile
        && profile
            .extensions
            .is_disabled(&ExtensionId::new(CLOUD_MEMBER))
    {
        return Err(CloudAccessError::Disabled {
            action,
            profile: profile.name.clone(),
        });
    }
    cloud
        .map(AsRef::as_ref)
        .ok_or(CloudAccessError::Absent { action })
}
