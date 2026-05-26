use std::fmt;

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    AgentId, HookId, MarketplaceId, McpServerId, PluginId, RouteId, SkillId,
};

use super::kinds::EntityKind;

/// Tagged-union reference to an authz target. Bundles the discriminator
/// (`EntityKind`) and the typed id so they can never drift apart on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum EntityRef {
    GatewayRoute(RouteId),
    McpServer(McpServerId),
    Plugin(PluginId),
    Agent(AgentId),
    Marketplace(MarketplaceId),
    Skill(SkillId),
    Hook(HookId),
}

impl EntityRef {
    #[must_use]
    pub const fn kind(&self) -> EntityKind {
        match self {
            Self::GatewayRoute(_) => EntityKind::GatewayRoute,
            Self::McpServer(_) => EntityKind::McpServer,
            Self::Plugin(_) => EntityKind::Plugin,
            Self::Agent(_) => EntityKind::Agent,
            Self::Marketplace(_) => EntityKind::Marketplace,
            Self::Skill(_) => EntityKind::Skill,
            Self::Hook(_) => EntityKind::Hook,
        }
    }

    #[must_use]
    pub fn id_str(&self) -> &str {
        match self {
            Self::GatewayRoute(id) => id.as_str(),
            Self::McpServer(id) => id.as_str(),
            Self::Plugin(id) => id.as_str(),
            Self::Agent(id) => id.as_str(),
            Self::Marketplace(id) => id.as_str(),
            Self::Skill(id) => id.as_str(),
            Self::Hook(id) => id.as_str(),
        }
    }
}

impl fmt::Display for EntityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.kind().as_str(), self.id_str())
    }
}
