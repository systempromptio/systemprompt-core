//! OAuth client identifier with classifier helpers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

crate::define_id!(ClientId);

impl ClientId {
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

    pub fn is_dcr(&self) -> bool {
        matches!(
            self.client_type(),
            ClientType::FirstParty | ClientType::ThirdParty
        )
    }

    pub fn is_cimd(&self) -> bool {
        self.0.starts_with("https://")
    }

    pub fn is_system(&self) -> bool {
        self.0.starts_with("sys_")
    }

    pub fn web() -> Self {
        Self("sp_web".to_owned())
    }

    pub fn cli() -> Self {
        Self("sp_cli".to_owned())
    }

    pub fn mobile_ios() -> Self {
        Self("sp_mobile_ios".to_owned())
    }

    pub fn mobile_android() -> Self {
        Self("sp_mobile_android".to_owned())
    }

    pub fn desktop() -> Self {
        Self("sp_desktop".to_owned())
    }

    pub fn bridge() -> Self {
        Self("sp_bridge".to_owned())
    }

    pub fn sync() -> Self {
        Self("sys_sync".to_owned())
    }

    pub fn system(service_name: &str) -> Self {
        Self(format!("sys_{service_name}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientType {
    Cimd,
    FirstParty,
    ThirdParty,
    System,
    Unknown,
}

impl ClientType {
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
