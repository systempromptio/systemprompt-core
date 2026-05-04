//! Session identifier (`sess_<uuid>`) and its originating-source enum.

crate::define_id!(SessionId, schema);

impl SessionId {
    /// Mints a fresh session identifier of the form `sess_<uuid>`.
    pub fn generate() -> Self {
        Self(format!("sess_{}", uuid::Uuid::new_v4()))
    }

    /// Returns the canonical `"system"` session identifier.
    pub fn system() -> Self {
        Self("system".to_string())
    }
}

/// Channel through which a session was initiated.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum SessionSource {
    /// Web SPA login.
    Web,
    /// Direct API client.
    Api,
    /// CLI invocation.
    Cli,
    /// OAuth authorization-code flow.
    Oauth,
    /// MCP transport session.
    Mcp,
    /// Co-working assistant session.
    Cowork,
    /// Source could not be determined.
    #[default]
    Unknown,
}

impl SessionSource {
    /// Returns the canonical lowercase string representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Api => "api",
            Self::Cli => "cli",
            Self::Oauth => "oauth",
            Self::Mcp => "mcp",
            Self::Cowork => "cowork",
            Self::Unknown => "unknown",
        }
    }

    /// Infers the [`SessionSource`] from a canonical first-party
    /// [`crate::ClientId`] prefix.
    pub fn from_client_id(client_id: &str) -> Self {
        match client_id {
            "sp_web" => Self::Web,
            "sp_cli" => Self::Cli,
            "sp_cowork" => Self::Cowork,
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
            "cowork" => Self::Cowork,
            _ => Self::Unknown,
        })
    }
}
