//! `google.rpc.ErrorInfo` details carried in every A2A JSON-RPC error.
//!
//! A2A 1.0 puts an array of `ErrorInfo` messages in `error.data`;
//! [`A2aErrorInfo`] is one entry and [`A2aErrorReason`] the closed set of
//! reasons this server reports.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use serde::Serialize;

pub const ERROR_INFO_TYPE_URL: &str = "type.googleapis.com/google.rpc.ErrorInfo";
pub const A2A_ERROR_DOMAIN: &str = "a2a-protocol.org";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum A2aErrorReason {
    TaskNotFound,
    TaskNotCancelable,
    PushNotificationNotSupported,
    UnsupportedOperation,
    ContentTypeNotSupported,
    InvalidAgentResponse,
    ExtendedAgentCardNotConfigured,
    ExtensionSupportRequired,
    VersionNotSupported,
    InvalidParams,
    InvalidRequest,
    MethodNotFound,
    Internal,
    Unauthenticated,
    Forbidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct A2aErrorInfo {
    #[serde(rename = "@type")]
    pub type_url: &'static str,
    pub reason: A2aErrorReason,
    pub domain: &'static str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl A2aErrorInfo {
    #[must_use]
    pub const fn new(reason: A2aErrorReason) -> Self {
        Self {
            type_url: ERROR_INFO_TYPE_URL,
            reason,
            domain: A2A_ERROR_DOMAIN,
            metadata: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}
