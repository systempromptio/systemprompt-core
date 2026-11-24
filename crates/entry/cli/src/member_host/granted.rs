//! The values the member host builds for a granted command: the identity
//! plane check, the services tree and the identity settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::cli::{CommandError, GrantedPlane, IdentityCliSettings, ServicesTree};
use systemprompt_manifest::profile::PlaneAuthority;
use systemprompt_manifest::services::BridgeReleasesSpec;

use crate::context::CommandContext;
use crate::kernel::KernelHost;

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("only a command identity-http owns reads the identity settings")]
pub(crate) struct IdentitySettingsRefused;

pub(super) fn identity_settings(ctx: &CommandContext) -> Result<IdentityCliSettings, CommandError> {
    let config = KernelHost::config(ctx)?;
    let security = &ctx.profile().map_err(CommandError::new)?.security;
    let bridge_release_window = ctx
        .services()
        .and_then(|services| {
            services
                .config()
                .gateway_config()
                .and_then(|gateway| gateway.bridge_releases.as_ref())
                .map(BridgeReleasesSpec::version_window)
        })
        .unwrap_or_default();
    Ok(IdentityCliSettings {
        api_key_max_lifetime_days: config.api_key_max_lifetime_days,
        archived_users_days: config.retention.archived_users_days,
        api_external_url: config.api_external_url.clone(),
        bridge_release_window,
        issuer: security.issuer.clone(),
        trusted_issuers: security.trusted_issuers.clone(),
    })
}

pub(super) async fn writable_plane(
    ctx: &CommandContext,
    plane: GrantedPlane,
) -> Result<(), CommandError> {
    let profile = ctx.profile().map_err(CommandError::new)?;
    if profile.config_planes.authority_for(plane.name()) != PlaneAuthority::File {
        return Ok(());
    }
    let app = ctx.app_context().await.map_err(CommandError::new)?;
    let config = app
        .kernel()
        .state()
        .get::<std::sync::Arc<systemprompt_manifest::Config>>()
        .map_err(CommandError::new)?;
    let planes = systemprompt_runtime::config_plane_set(app.admitted(), config)
        .map_err(CommandError::new)?;
    planes.writer(plane.name()).map_err(CommandError::new)?;
    Ok(())
}

pub(super) fn granted_services_tree(ctx: &CommandContext) -> Result<ServicesTree, CommandError> {
    let root = KernelHost::services_root(ctx)?.path;
    let profile = ctx.profile().map_err(CommandError::new)?;
    Ok(ServicesTree::new(
        root,
        profile.server.api_external_url.clone(),
        profile.site.github_link.clone(),
    ))
}
