mod session;
mod store;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::TenantId;

pub use session::{CliSession, CliSessionBuilder};
pub use store::SessionStore;

pub const LOCAL_SESSION_KEY: &str = "local";

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum SessionKey {
    Local,
    Tenant(TenantId),
}

impl SessionKey {
    #[must_use]
    pub fn from_tenant_id(tenant_id: Option<&str>) -> Self {
        tenant_id.map_or(Self::Local, |id| Self::Tenant(TenantId::new(id)))
    }

    #[must_use]
    pub fn as_storage_key(&self) -> String {
        match self {
            Self::Local => LOCAL_SESSION_KEY.to_string(),
            Self::Tenant(id) => format!("tenant_{}", id),
        }
    }

    #[must_use]
    pub fn tenant_id(&self) -> Option<&TenantId> {
        match self {
            Self::Local => None,
            Self::Tenant(id) => Some(id),
        }
    }

    #[must_use]
    pub fn tenant_id_str(&self) -> Option<&str> {
        self.tenant_id().map(TenantId::as_str)
    }

    #[must_use]
    pub const fn is_local(&self) -> bool {
        matches!(self, Self::Local)
    }
}

impl std::fmt::Display for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Tenant(id) => write!(f, "tenant:{}", id),
        }
    }
}
