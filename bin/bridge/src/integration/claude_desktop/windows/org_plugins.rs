//! The org-plugins directory a Claude Desktop policy install depends on:
//! provisioned and verified when elevated, required to exist otherwise.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::install::elevated_job::ElevateError;
use crate::integration::host_app::ProfileInstalled;

#[derive(Debug, thiserror::Error)]
#[error("org-plugins provisioning failed: {0}")]
struct ProvisionFailed(#[source] ElevateError);

pub(super) fn require_org_plugins_provisioned(elevated: bool) -> std::io::Result<ProfileInstalled> {
    let org = crate::install::elevated_job::ElevatedJob::org_plugins_for_current_user()?;
    if elevated {
        crate::install::elevated_job::provision_org_plugins(&org.path, &org.grant_user)
            .map_err(|e| std::io::Error::other(ProvisionFailed(e)))?;
        // Why: PermissionDenied is the check's verdict (the unelevated user
        // lacks Modify), not a failure to run it — that must fail the install.
        Ok(match crate::windows_acl::verify_modify_tree(&org.path) {
            Ok(()) => ProfileInstalled::ok(),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Err(e),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    path = %org.path.display(),
                    "org-plugins provisioned; Modify verification could not run"
                );
                ProfileInstalled::with_warning(format!(
                    "org-plugins provisioned at {} but the Modify check could not run ({e}); \
                     run `doctor` to confirm the grant",
                    org.path.display()
                ))
            },
        })
    } else if org.path.is_dir() {
        Ok(ProfileInstalled::ok())
    } else {
        Err(std::io::Error::other(format!(
            "{} is not provisioned; run install --apply as Administrator",
            org.path.display()
        )))
    }
}
