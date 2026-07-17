//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::{BearerToken, CertFingerprint};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use systemprompt_models::bridge::profile::{
    BridgeProfileResponse as BridgeProfile, ProviderHealth,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtlsRequest {
    pub device_cert_fingerprint: CertFingerprint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExchangeRequest {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPatRequest {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DevicePatResponse {
    pub pat: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: BearerToken,
    pub ttl: u64,
    #[serde(default, with = "header_map_serde")]
    pub headers: HashMap<http::HeaderName, http::HeaderValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperOutput {
    pub token: BearerToken,
    pub ttl: u64,
    #[serde(default, with = "header_map_serde")]
    pub headers: HashMap<http::HeaderName, http::HeaderValue>,
}

impl From<AuthResponse> for HelperOutput {
    fn from(r: AuthResponse) -> Self {
        Self {
            token: r.token,
            ttl: r.ttl,
            headers: r.headers,
        }
    }
}

mod header_map_serde {
    use http::{HeaderName, HeaderValue};
    use serde::de::{Deserializer, MapAccess, Visitor};
    use serde::ser::{SerializeMap, Serializer};
    use std::collections::HashMap;
    use std::fmt;

    pub(super) fn serialize<S: Serializer>(
        map: &HashMap<HeaderName, HeaderValue>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_map(Some(map.len()))?;
        for (name, value) in map {
            let value_str = value
                .to_str()
                .map_err(|e| serde::ser::Error::custom(format!("non-ascii header value: {e}")))?;
            out.serialize_entry(name.as_str(), value_str)?;
        }
        out.end()
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<HashMap<HeaderName, HeaderValue>, D::Error> {
        struct HeaderMapVisitor;

        impl<'de> Visitor<'de> for HeaderMapVisitor {
            type Value = HashMap<HeaderName, HeaderValue>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a map of HTTP header names to values")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
                let mut map = HashMap::new();
                while let Some((key, value)) = access.next_entry::<String, String>()? {
                    let name: HeaderName = key.parse().map_err(|e| {
                        serde::de::Error::custom(format!("invalid header name: {e}"))
                    })?;
                    let value: HeaderValue = value.parse().map_err(|e| {
                        serde::de::Error::custom(format!("invalid header value: {e}"))
                    })?;
                    map.insert(name, value);
                }
                Ok(map)
            }
        }

        deserializer.deserialize_map(HeaderMapVisitor)
    }
}
