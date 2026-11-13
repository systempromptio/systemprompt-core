//! The primitives each `ProtectionLayer` is applied with: the site-auth gate
//! over the static pages and the refusal of file-managed client writes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use systemprompt_extension::{LoaderError, SiteAuthConfig};
use systemprompt_models::ApiError;
use systemprompt_oauth_issuance::IDENTITY_PLANE;
use systemprompt_runtime::AppContext;
use systemprompt_security::keys::SigningKeys;

use crate::error::ApiHttpError;
use crate::services::middleware::site_auth::{SiteAuthSession, site_auth_gate};
use crate::services::session_cookie::SessionCookie;

const CLIENTS_PATH: &str = "/clients";

/// The site-auth gate an extension's [`SiteAuthConfig`] installs over the
/// static pages.
#[derive(Clone)]
pub struct SiteAuthGuard {
    pub config: SiteAuthConfig,
    pub issuer: Arc<str>,
    pub keys: SigningKeys,
    pub cookie: SessionCookie,
}

impl std::fmt::Debug for SiteAuthGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SiteAuthGuard")
            .field("config", &self.config)
            .field("issuer", &self.issuer)
            .finish_non_exhaustive()
    }
}

impl SiteAuthGuard {
    pub fn from_context(ctx: &AppContext) -> Result<Option<Self>, LoaderError> {
        let config = ctx
            .extension_registry()
            .enabled_site_auth(&ctx.profile().extensions.disabled)?;
        Ok(config.map(|config| Self {
            config,
            issuer: Arc::from(ctx.config().jwt_issuer.as_str()),
            keys: ctx.signing_keys().clone(),
            cookie: SessionCookie::from_config(ctx.config()),
        }))
    }

    pub(super) async fn gate(self, request: Request, next: Next) -> Response {
        site_auth_gate(
            request,
            next,
            self.config,
            SiteAuthSession {
                issuer: &self.issuer,
                keys: &self.keys,
                cookie: self.cookie,
            },
        )
        .await
    }
}

/// Refuses the client-management writes (`POST`/`PUT`/`DELETE` under
/// `/clients`) while the identity config plane is file-managed.
#[derive(Clone)]
pub struct ClientWritesGuard {
    ctx: AppContext,
}

impl std::fmt::Debug for ClientWritesGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientWritesGuard").finish_non_exhaustive()
    }
}

impl ClientWritesGuard {
    #[must_use]
    pub const fn new(ctx: AppContext) -> Self {
        Self { ctx }
    }

    pub(super) async fn refuse(self, request: Request, next: Next) -> Response {
        if request.uri().path().starts_with(CLIENTS_PATH) && !request.method().is_safe() {
            let refused = self
                .ctx
                .config_planes()
                .and_then(|planes| planes.writer(IDENTITY_PLANE).map(|_| ()));
            if let Err(error) = refused {
                return ApiHttpError::from(ApiError::from(error)).into_response();
            }
        }
        next.run(request).await
    }
}

/// The primitives each `ProtectionLayer` is applied with.
#[derive(Clone, Default)]
pub struct ProtectionGuards {
    pub site_auth: Option<SiteAuthGuard>,
    pub client_writes: Option<ClientWritesGuard>,
}

impl std::fmt::Debug for ProtectionGuards {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtectionGuards")
            .field("site_auth", &self.site_auth)
            .field("client_writes", &self.client_writes)
            .finish()
    }
}

impl ProtectionGuards {
    pub fn from_context(ctx: &AppContext) -> Result<Self, LoaderError> {
        Ok(Self {
            site_auth: SiteAuthGuard::from_context(ctx)?,
            client_writes: Some(ClientWritesGuard::new(ctx.clone())),
        })
    }
}
