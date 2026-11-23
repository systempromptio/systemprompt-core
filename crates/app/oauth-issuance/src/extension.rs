//! The `identity` member: the `identity` config plane, the
//! [`IdentityClientSecrets`] handle it reads declared client secrets through
//! (built here from the sealed secret store), the secret names it reads
//! ([`IdentitySecretReferences`], refused to every install), and core's
//! generic OIDC SSO provider. This crate owns no tables; the plane writes users
//! through the users domain's repository and static OAuth clients through the
//! OAuth domain's.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_manifest::services::root::ActiveServicesRoot;
use systemprompt_traits::{SecretReferences, SecretStore};

use crate::contract::SsoSlot;
use crate::identity::{IdentityClientSecrets, IdentityPlane, IdentitySecretReferences};
use crate::sso::{SsoHookContext, oidc};

#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityExtension;

impl Extension for IdentityExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "identity",
            name: "Identity",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.provide(
            |r: &Resolved<'_>| -> Result<IdentityClientSecrets, ProvideError> {
                Ok(IdentityClientSecrets::new(r.get::<Arc<dyn SecretStore>>()?))
            },
        )
        .config_plane(Arc::new(IdentityPlane))
        .secret_references(
            |r: &Resolved<'_>| -> Result<Arc<dyn SecretReferences>, FactoryError> {
                let root = r.get::<ActiveServicesRoot>()?;
                Ok(Arc::new(IdentitySecretReferences::over(root.path)))
            },
        );
        c.slot::<SsoSlot>().provider(|r: &Resolved<'_>| {
            oidc::factory(&SsoHookContext::from_resolved(r)?)
                .map_err(|e| FactoryError::Failed(Box::new(e)))
        });
        Ok(())
    }
}
