//! OAuth client identifier with classifier helpers.

crate::define_id!(ClientId);

impl ClientId {
    /// Returns the [`ClientType`] inferred from the identifier prefix.
    pub fn client_type(&self) -> ClientType {
        if self.0.starts_with("https://") {
            ClientType::Cimd
        } else if self.0.starts_with("sp_") {
            ClientType::FirstParty
        } else if self.0.starts_with("client_") {
            ClientType::ThirdParty
        } else if self.0.starts_with("sys_") {
            ClientType::System
        } else {
            ClientType::Unknown
        }
    }

    /// Returns true if this is a dynamically-registered client (first- or
    /// third-party).
    pub fn is_dcr(&self) -> bool {
        matches!(
            self.client_type(),
            ClientType::FirstParty | ClientType::ThirdParty
        )
    }

    /// Returns true if this is a CIMD (Client Identifier Metadata Document)
    /// URL-form client.
    pub fn is_cimd(&self) -> bool {
        self.0.starts_with("https://")
    }

    /// Returns true if this is a system-issued internal client identifier.
    pub fn is_system(&self) -> bool {
        self.0.starts_with("sys_")
    }

    /// Canonical web-app client identifier.
    pub fn web() -> Self {
        Self("sp_web".to_string())
    }

    /// Canonical CLI client identifier.
    pub fn cli() -> Self {
        Self("sp_cli".to_string())
    }

    /// Canonical iOS mobile client identifier.
    pub fn mobile_ios() -> Self {
        Self("sp_mobile_ios".to_string())
    }

    /// Canonical Android mobile client identifier.
    pub fn mobile_android() -> Self {
        Self("sp_mobile_android".to_string())
    }

    /// Canonical desktop client identifier.
    pub fn desktop() -> Self {
        Self("sp_desktop".to_string())
    }

    /// Canonical co-work session client identifier.
    pub fn cowork() -> Self {
        Self("sp_cowork".to_string())
    }

    /// Constructs a system client identifier of the form `sys_<service_name>`.
    pub fn system(service_name: &str) -> Self {
        Self(format!("sys_{service_name}"))
    }
}

/// Coarse client classification derived from a [`ClientId`] prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientType {
    /// CIMD URL-form client.
    Cimd,
    /// systemprompt.io first-party client (`sp_*`).
    FirstParty,
    /// Third-party DCR client (`client_*`).
    ThirdParty,
    /// Internal system client (`sys_*`).
    System,
    /// Unrecognised prefix.
    Unknown,
}

impl ClientType {
    /// Returns the canonical lowercase string representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cimd => "cimd",
            Self::FirstParty => "firstparty",
            Self::ThirdParty => "thirdparty",
            Self::System => "system",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for ClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
