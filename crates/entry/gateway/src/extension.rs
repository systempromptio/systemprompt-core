//! [`GatewayHttpExtension`] (`gateway-http`): the gateway's handler surface.
//!
//! With the `http` feature the member provides its
//! [`GatewaySurface`](systemprompt_gateway::GatewaySurface) (open: a builder
//! that holds nothing) and, sealed, the
//! [`GatewayHttpRepositories`](crate::http::state::GatewayHttpRepositories) it
//! builds here — the API-key check over the user repository, with the key
//! lifetime the runtime config sets, and the context materializer over the
//! context repository. The API mount resolves the surface's state scoped to
//! this member, so the engines, at-rest material and session mint reach it only
//! through their conferral rows.
//!
//! | profile disables | inference, token count, `/otel` | `/me/provider-credentials` | bridge Drive (`/bridge/drive/*`) |
//! |---|---|---|---|
//! | `gateway-http` | 404 (confined) | 404 (confined) | mounted (the API's own routes) |
//! | `ai` | mounted | 404: `ai` confers the credential view | mounted |
//! | `documents` | mounted | mounted | 404: no Drive token view or document store |
//! | `gateway` | 404: no engines are built and nothing is mounted | 404 | 404 |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const GATEWAY_HTTP_EXTENSION: &str = "gateway-http";

#[derive(Debug, Clone, Copy, Default)]
pub struct GatewayHttpExtension;

impl Extension for GatewayHttpExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: GATEWAY_HTTP_EXTENSION,
            name: "Gateway HTTP",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    #[cfg_attr(
        not(feature = "http"),
        expect(
            unused_variables,
            reason = "the member contributes its surface only with the http feature"
        )
    )]
    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        surface::contribute(c);
        Ok(())
    }
}

#[cfg(feature = "http")]
mod surface {
    use std::sync::Arc;

    use systemprompt_database::DbPool;
    use systemprompt_extension::prelude::*;
    use systemprompt_extension::profile_usage::DynProfileUsageSource;
    use systemprompt_gateway::DynGatewaySurface;
    use systemprompt_manifest::Config;
    use systemprompt_users::{ApiKeyLifetime, ApiKeyService, UserRepository};

    use crate::http::GatewayHttpSurface;
    use crate::http::bridge::BridgeRepositories;
    use crate::http::state::{GatewayApiKeys, GatewayHttpRepositories};
    use systemprompt_oauth::repository::{BridgeHostPrefsRepository, BridgeSessionRepository};

    pub(super) fn contribute(c: &mut Contributions<'_>) {
        c.provide(
            |_r: &Resolved<'_>| -> Result<DynGatewaySurface, ProvideError> {
                Ok(Arc::new(GatewayHttpSurface))
            },
        )
        .provide_sealed_after_governance(repositories);
    }

    fn repositories(r: &Resolved<'_>) -> Result<Arc<GatewayHttpRepositories>, ProvideError> {
        let db = r.get::<DbPool>()?;
        let config = r.get::<Arc<Config>>()?;
        let users = Arc::new(UserRepository::new(&db));
        Ok(Arc::new(GatewayHttpRepositories {
            api_keys: GatewayApiKeys::new(ApiKeyService::new(
                Arc::clone(&users),
                ApiKeyLifetime::from_max_days(config.api_key_max_lifetime_days),
            )),
            bridge: BridgeRepositories {
                users,
                host_prefs: BridgeHostPrefsRepository::new(&db),
                sessions: BridgeSessionRepository::new(&db),
                profile_usage: r.optional::<DynProfileUsageSource>(),
            },
        }))
    }
}
