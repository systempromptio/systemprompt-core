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
            ProtocolBinding::JsonRpc => "JSONRPC".to_owned(),
            ProtocolBinding::Grpc => "GRPC".to_owned(),
            ProtocolBinding::HttpJson => "HTTP+JSON".to_owned(),
        }
    }
}

impl std::str::FromStr for ProtocolBinding {
    type Err = crate::errors::ParseEnumError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "JSONRPC" => Ok(Self::JsonRpc),
            "GRPC" => Ok(Self::Grpc),
            "HTTP+JSON" => Ok(Self::HttpJson),
            _ => Err(crate::errors::ParseEnumError::new("protocol_binding", s)),
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
