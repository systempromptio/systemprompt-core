use systemprompt_client::SystempromptClient;
use systemprompt_identifiers::{AgentName, JwtToken, SessionId, TraceId};
use systemprompt_models::profile_bootstrap::ProfileBootstrap;
use systemprompt_models::{Profile, RequestContext};

use crate::api_client::CloudApiClient;
use crate::cli_session::CliSession;
use crate::credentials::CloudCredentials;
use crate::error::{CloudError, CloudResult};
use crate::paths::{get_cloud_paths, CloudPath};
use crate::tenants::{StoredTenant, TenantStore};

#[derive(Debug, Clone)]
pub struct ResolvedTenant {
    pub id: String,
    pub name: String,
    pub app_id: Option<String>,
    pub hostname: Option<String>,
    pub region: Option<String>,
}

impl From<StoredTenant> for ResolvedTenant {
    fn from(tenant: StoredTenant) -> Self {
        Self {
            id: tenant.id,
            name: tenant.name,
            app_id: tenant.app_id,
            hostname: tenant.hostname,
            region: tenant.region,
        }
    }
}

#[derive(Debug)]
pub struct CloudContext {
    pub credentials: CloudCredentials,
    pub profile: Option<&'static Profile>,
    pub tenant: Option<ResolvedTenant>,
    pub api_client: CloudApiClient,
    session: Option<CliSession>,
}

impl CloudContext {
    pub fn new_authenticated() -> CloudResult<Self> {
        let cloud_paths = get_cloud_paths().map_err(|_| CloudError::NotAuthenticated)?;
        let creds_path = cloud_paths.resolve(CloudPath::Credentials);
        let credentials = CloudCredentials::load_and_validate_from_path(&creds_path)
            .map_err(|_| CloudError::NotAuthenticated)?;

        let session_path = cloud_paths.resolve(CloudPath::CliSession);
        let session = CliSession::load_from_path(&session_path).ok();

        let api_client = CloudApiClient::new(&credentials.api_url, &credentials.api_token);

        Ok(Self {
            credentials,
            profile: None,
            tenant: None,
            api_client,
            session,
        })
    }

    pub fn with_profile(mut self) -> CloudResult<Self> {
        let profile = ProfileBootstrap::get().map_err(|e| CloudError::ProfileRequired {
            message: e.to_string(),
        })?;

        self.profile = Some(profile);

        if let Some(ref cloud_config) = profile.cloud {
            if let Some(ref tenant_id) = cloud_config.tenant_id {
                self.tenant = Self::resolve_tenant(tenant_id)?;
            }
        }

        Ok(self)
    }

    fn resolve_tenant(tenant_id: &str) -> CloudResult<Option<ResolvedTenant>> {
        let cloud_paths = get_cloud_paths().map_err(|_| CloudError::TenantsNotSynced)?;
        let tenants_path = cloud_paths.resolve(CloudPath::Tenants);

        if !tenants_path.exists() {
            return Ok(None);
        }

        let store =
            TenantStore::load_from_path(&tenants_path).map_err(|_| CloudError::TenantsNotSynced)?;

        store.find_tenant(tenant_id).map_or_else(
            || {
                Err(CloudError::TenantNotFound {
                    tenant_id: tenant_id.to_string(),
                })
            },
            |tenant| Ok(Some(ResolvedTenant::from(tenant.clone()))),
        )
    }

    pub fn tenant_id(&self) -> CloudResult<&str> {
        self.tenant
            .as_ref()
            .map(|t| t.id.as_str())
            .ok_or(CloudError::TenantNotConfigured)
    }

    pub fn app_id(&self) -> CloudResult<&str> {
        self.tenant
            .as_ref()
            .and_then(|t| t.app_id.as_deref())
            .ok_or(CloudError::AppNotConfigured)
    }

    #[must_use]
    pub fn tenant_name(&self) -> &str {
        self.tenant.as_ref().map_or("unknown", |t| t.name.as_str())
    }

    #[must_use]
    pub fn hostname(&self) -> Option<&str> {
        self.tenant.as_ref().and_then(|t| t.hostname.as_deref())
    }

    pub fn profile(&self) -> CloudResult<&'static Profile> {
        self.profile.ok_or_else(|| CloudError::ProfileRequired {
            message: "Profile not loaded in context".into(),
        })
    }

    #[must_use]
    pub const fn has_tenant(&self) -> bool {
        self.tenant.is_some()
    }

    #[must_use]
    pub const fn session(&self) -> Option<&CliSession> {
        self.session.as_ref()
    }

    pub async fn get_or_create_request_context(
        &mut self,
        agent_name: &str,
    ) -> CloudResult<RequestContext> {
        let cloud_paths = get_cloud_paths().map_err(|_| CloudError::NotAuthenticated)?;
        let session_path = cloud_paths.resolve(CloudPath::CliSession);

        let (context_id, session_id) = if let Some(ref mut session) = self.session {
            session.touch();
            let _ = session.save_to_path(&session_path);
            (session.context_id.clone(), session.session_id.clone())
        } else {
            let client = SystempromptClient::new(&self.credentials.api_url)
                .map_err(|e| CloudError::ApiError {
                    message: e.to_string(),
                })?
                .with_token(JwtToken::new(&self.credentials.api_token));

            let context_id = client.fetch_or_create_context().await.map_err(|e| {
                CloudError::ApiError {
                    message: format!("Failed to create context: {}", e),
                }
            })?;

            let session_id = SessionId::new(uuid::Uuid::new_v4().to_string());
            let session = CliSession::new(context_id.clone(), session_id.clone());
            let _ = session.save_to_path(&session_path);
            self.session = Some(session);

            (context_id, session_id)
        };

        let trace_id = TraceId::new(uuid::Uuid::new_v4().to_string());

        Ok(
            RequestContext::new(session_id, trace_id, context_id, AgentName::new(agent_name))
                .with_auth_token(&self.credentials.api_token),
        )
    }

    pub fn clear_session(&mut self) -> CloudResult<()> {
        let cloud_paths = get_cloud_paths().map_err(|_| CloudError::NotAuthenticated)?;
        let session_path = cloud_paths.resolve(CloudPath::CliSession);
        CliSession::delete_from_path(&session_path).map_err(|e| CloudError::ApiError {
            message: e.to_string(),
        })?;
        self.session = None;
        Ok(())
    }
}
