use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtocolBinding {
    #[serde(rename = "JSONRPC")]
    #[default]
    JsonRpc,
    #[serde(rename = "GRPC")]
    Grpc,
    #[serde(rename = "HTTP+JSON")]
    HttpJson,
}

pub type TransportProtocol = ProtocolBinding;

impl From<ProtocolBinding> for String {
    fn from(transport: ProtocolBinding) -> Self {
        match transport {
            ProtocolBinding::JsonRpc => "JSONRPC".to_string(),
            ProtocolBinding::Grpc => "GRPC".to_string(),
            ProtocolBinding::HttpJson => "HTTP+JSON".to_string(),
        }
    }
}

impl std::str::FromStr for ProtocolBinding {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "JSONRPC" => Ok(Self::JsonRpc),
            "GRPC" => Ok(Self::Grpc),
            "HTTP+JSON" => Ok(Self::HttpJson),
            _ => Err(anyhow::anyhow!("Invalid protocol binding: {}", s)),
        }
    }
}

impl std::fmt::Display for ProtocolBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonRpc => write!(f, "JSONRPC"),
            Self::Grpc => write!(f, "GRPC"),
            Self::HttpJson => write!(f, "HTTP+JSON"),
        }
    }
}

impl ProtocolBinding {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::JsonRpc => "JSONRPC",
            Self::Grpc => "GRPC",
            Self::HttpJson => "HTTP+JSON",
        }
    }
}
