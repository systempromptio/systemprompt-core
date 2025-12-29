use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum TransportProtocol {
    #[serde(rename = "JSONRPC")]
    #[default]
    JsonRpc,
    #[serde(rename = "GRPC")]
    Grpc,
    #[serde(rename = "HTTP+JSON")]
    HttpJson,
}

impl From<TransportProtocol> for String {
    fn from(transport: TransportProtocol) -> Self {
        match transport {
            TransportProtocol::JsonRpc => "JSONRPC".to_string(),
            TransportProtocol::Grpc => "GRPC".to_string(),
            TransportProtocol::HttpJson => "HTTP+JSON".to_string(),
        }
    }
}

impl std::str::FromStr for TransportProtocol {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "JSONRPC" => Ok(Self::JsonRpc),
            "GRPC" => Ok(Self::Grpc),
            "HTTP+JSON" => Ok(Self::HttpJson),
            _ => Err(anyhow::anyhow!("Invalid transport protocol: {}", s)),
        }
    }
}

impl std::fmt::Display for TransportProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JsonRpc => write!(f, "JSONRPC"),
            Self::Grpc => write!(f, "GRPC"),
            Self::HttpJson => write!(f, "HTTP+JSON"),
        }
    }
}

impl TransportProtocol {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::JsonRpc => "JSONRPC",
            Self::Grpc => "GRPC",
            Self::HttpJson => "HTTP+JSON",
        }
    }
}
