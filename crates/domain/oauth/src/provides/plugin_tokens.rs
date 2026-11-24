//! The sealed plugin-token minter the oauth member confers on
//! `identity-http` (`identity-plugin-tokens`): an admin lookup over the users
//! member's sealed directory, a CLI session row over its sealed session store,
//! and [`PluginTokenService`]'s one token shape over the kernel token
//! authority.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::prelude::{ProvideError, Resolved};
use systemprompt_identifiers::SessionSource;
use systemprompt_manifest::Config;
use systemprompt_models::auth::TokenAuthority;
use systemprompt_traits::{
    AdminAccount, AuthUser, DynPluginTokenMinter, DynSessionStore, DynUserProvider,
    MintedPluginToken, PLUGIN_TOKEN_MAX_DAYS, PluginTokenHolder, PluginTokenMintError,
    PluginTokenMinter, PluginTokenRequest, SessionAnalytics, SessionProvider,
};

use crate::services::SessionCreationService;
use crate::services::plugin_token::{PluginTokenService, PluginTokenSubject};

struct OauthPluginTokenMinter {
    sessions: SessionCreationService,
    users: DynUserProvider,
    authority: Arc<dyn TokenAuthority>,
}

fn failed(e: impl std::error::Error + Send + Sync + 'static) -> PluginTokenMintError {
    PluginTokenMintError::Failed(Box::new(e))
}

fn is_admin(user: &AuthUser) -> bool {
    user.roles
        .iter()
        .any(|role| role == AdminAccount::ADMIN_ROLE)
}

impl OauthPluginTokenMinter {
    async fn holder(&self, holder: &PluginTokenHolder) -> Result<AuthUser, PluginTokenMintError> {
        let (found, named) = match holder {
            PluginTokenHolder::Email(email) => (
                self.users.find_by_email(email).await.map_err(failed)?,
                email.clone(),
            ),
            PluginTokenHolder::User(id) => (
                self.users.find_by_id(id).await.map_err(failed)?,
                id.as_str().to_owned(),
            ),
        };
        let user = found.ok_or(PluginTokenMintError::UserNotFound(named))?;
        if !user.is_active || !is_admin(&user) {
            return Err(PluginTokenMintError::NotAdmin(user.email));
        }
        Ok(user)
    }
}

#[async_trait]
impl PluginTokenMinter for OauthPluginTokenMinter {
    async fn mint(
        &self,
        request: PluginTokenRequest,
    ) -> Result<MintedPluginToken, PluginTokenMintError> {
        if request.duration_days == 0 || request.duration_days > PLUGIN_TOKEN_MAX_DAYS {
            return Err(PluginTokenMintError::Duration(request.duration_days));
        }
        let user = self.holder(&request.holder).await?;
        let session_id = self
            .sessions
            .create_authenticated_session_with_ttl(
                &user.id,
                &SessionAnalytics::default(),
                SessionSource::Cli,
                chrono::Duration::days(i64::from(request.duration_days)),
            )
            .await
            .map_err(failed)?;
        let issued = PluginTokenService::issue(
            PluginTokenSubject {
                id: user.id.clone(),
                username: user.name,
                email: user.email.clone(),
            },
            self.authority.as_ref(),
            request.plugin_id,
            request.duration_days,
            &session_id,
        )
        .map_err(failed)?;
        Ok(MintedPluginToken {
            user_id: user.id,
            email: user.email,
            jti: issued.jti,
            token: issued.token,
        })
    }
}

pub(crate) fn plugin_token_minter(r: &Resolved<'_>) -> Result<DynPluginTokenMinter, ProvideError> {
    let (Some(authority), Some(store), Some(users)) = (
        r.optional::<Arc<dyn TokenAuthority>>(),
        r.optional::<DynSessionStore>(),
        r.optional::<DynUserProvider>(),
    ) else {
        return Err(ProvideError::Withheld);
    };
    let sessions: Arc<dyn SessionProvider> = store;
    let config = r.get::<Arc<Config>>()?;
    Ok(Arc::new(OauthPluginTokenMinter {
        sessions: SessionCreationService::new(
            sessions,
            Arc::clone(&users),
            Arc::clone(&authority),
            config.jwt_access_token_expiration,
        ),
        users,
        authority,
    }))
}
