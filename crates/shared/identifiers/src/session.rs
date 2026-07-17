//! Session identifier (`sess_<uuid>`) and its originating-source enum.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(SessionId, schema);

impl SessionId {
    pub fn generate() -> Self {
        Self(format!("sess_{}", uuid::Uuid::new_v4()))
    }

    pub fn system() -> Self {
        Self("system".to_owned())
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum SessionSource {
    Web,
    Api,
    Cli,
    Oauth,
    Mcp,
    Bridge,
    #[default]
    Unknown,
}

impl SessionSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Api => "api",
            Self::Cli => "cli",
            Self::Oauth => "oauth",
            Self::Mcp => "mcp",
            Self::Bridge => "bridge",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_client_id(client_id: &crate::ClientId) -> Self {
        match client_id.as_str() {
            "sp_web" => Self::Web,
            "sp_cli" => Self::Cli,
            "sp_bridge" => Self::Bridge,
            _ => Self::Unknown,
        }
    }
}

impl std::fmt::Display for SessionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for SessionSource {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "web" => Self::Web,
            "api" => Self::Api,
            "cli" => Self::Cli,
            "oauth" => Self::Oauth,
            "mcp" => Self::Mcp,
            "bridge" => Self::Bridge,
            _ => Self::Unknown,
        })
    }
}
