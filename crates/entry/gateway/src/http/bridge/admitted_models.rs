//! [`AdmittedModelsIssuer`]: the issuer of the conferred per-caller model
//! admission, which `gateway-http` provides sealed under the
//! `admitted-models` row of
//! `systemprompt_extension::reserved::INSTALL_CONFERRALS`.
//!
//! A member's handle answers which models the instance's rules admit for the
//! caller a bearer proves, by the bridge profile's own check: the bearer is
//! verified by the request authenticator (`decode_for_gateway`, as the
//! bridge profile does), the catalog's advertised models are narrowed to
//! those with a configured provider key, then to those the authz hook allows
//! this principal ([`ModelAdmission`]), decided through the host's sealed
//! authz hook cell, which denies until the host binds it. The answer is a
//! subset of the caller's candidates; no hook, secret name or principal leaves
//! core. A bearer core does not verify is refused, never answered with a
//! default. A child host confers nothing (`ConferralsWithheld`), so the issuer
//! is withheld there; so it is on a composition with no request
//! authenticator or no bridge settings view, so the conferral reads
//! `Withheld` rather than failing the composition.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_gateway::surface::views::GatewaySettingsView;
use systemprompt_governance::authz::{AuthzHookCell, LateAuthzHook, SharedAuthzHook};
use systemprompt_identifiers::{ExtensionId, JwtToken, ModelId};
use systemprompt_manifest::bridge_profile;
use systemprompt_manifest::services::ServicesView;
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_traits::{
    AdmittedModels, AdmittedModelsError, ConferralIssuer, DynConferralIssuer, MemberGrant,
};

use super::admission::{AdmissionPrincipal, ModelAdmission};

/// The sealed issuer of every member's model admission handle.
#[derive(Clone)]
pub struct AdmittedModelsIssuer {
    authenticator: DynRequestAuthenticator,
    services: ServicesView,
    authz_hook: SharedAuthzHook,
    settings: Arc<GatewaySettingsView>,
}

impl std::fmt::Debug for AdmittedModelsIssuer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdmittedModelsIssuer(<redacted>)")
    }
}

impl AdmittedModelsIssuer {
    pub(crate) fn provide(
        r: &Resolved<'_>,
    ) -> Result<DynConferralIssuer<dyn AdmittedModels>, ProvideError> {
        if r.optional::<systemprompt_extension::ConferralsWithheld>()
            .is_some()
        {
            return Err(ProvideError::Withheld);
        }
        let settings = r
            .optional::<Option<Arc<GatewaySettingsView>>>()
            .flatten()
            .ok_or(ProvideError::Withheld)?;
        Ok(Arc::new(Self {
            authenticator: r
                .optional::<DynRequestAuthenticator>()
                .ok_or(ProvideError::Withheld)?,
            services: r.get::<ServicesView>()?,
            authz_hook: Arc::new(LateAuthzHook::new(r.get::<AuthzHookCell>()?)),
            settings,
        }))
    }

    async fn admitted(
        &self,
        bearer: &JwtToken,
        candidates: Option<&[ModelId]>,
    ) -> Result<Vec<ModelId>, AdmittedModelsError> {
        let (claims, user) = self
            .authenticator
            .decode_for_gateway(bearer)
            .await
            .map_err(|source| AdmittedModelsError::Unauthenticated(Box::new(source)))?;
        let catalog = &self.services.catalog;
        self.services
            .config()
            .gateway_config()
            .filter(|gateway| gateway.enabled)
            .ok_or(AdmittedModelsError::GatewayDisabled)?;
        let secret_present = |name: &str| self.settings.credential_configured(name);
        let profile = bridge_profile::build(
            bridge_profile::BridgeProfileParams {
                inference_gateway_base_url: String::new(),
                auth_scheme: String::new(),
                organization_uuid: None,
                catalog,
            },
            &bridge_profile::CredentialFacts {
                secret_present: &secret_present,
                personal_keys: &BTreeSet::new(),
                presents_own_credential: false,
            },
        );
        let mut models = profile.models;
        if let Some(candidates) = candidates {
            models.retain(|model| candidates.contains(model));
        }
        let principal = AdmissionPrincipal {
            user_id: claims.user_id,
            roles: user.roles,
            client_id: claims.client_id,
            session_id: claims.session_id,
        };
        Ok(ModelAdmission {
            catalog,
            hook: &self.authz_hook,
            principal: &principal,
        }
        .retain(models)
        .await)
    }
}

impl ConferralIssuer<dyn AdmittedModels> for AdmittedModelsIssuer {
    fn issue(&self, grant: &MemberGrant) -> Arc<dyn AdmittedModels> {
        Arc::new(MemberAdmittedModels {
            issuer: self.clone(),
            member: grant.member.clone(),
        })
    }
}

struct MemberAdmittedModels {
    issuer: AdmittedModelsIssuer,
    member: ExtensionId,
}

impl std::fmt::Debug for MemberAdmittedModels {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberAdmittedModels")
            .field("member", &self.member)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl AdmittedModels for MemberAdmittedModels {
    async fn admitted(
        &self,
        bearer: &JwtToken,
        candidates: Option<&[ModelId]>,
    ) -> Result<Vec<ModelId>, AdmittedModelsError> {
        self.issuer.admitted(bearer, candidates).await
    }
}
