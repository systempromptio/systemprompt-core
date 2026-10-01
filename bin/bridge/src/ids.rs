//! Identifier and secret types the bridge carries.
//!
//! Every identity is the canonical `systemprompt_identifiers` type,
//! re-exported here with the manifest value types from
//! `systemprompt_models::bridge::ids`. The secrets (`PatToken`, `BearerToken`,
//! the loopback and hook tokens) stay bridge-local: they zeroize on drop and
//! hand their bytes out only through `into_inner`/`as_str`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub use systemprompt_identifiers::{
    CommsMessageId, DeploymentOrganizationUuid, HookSessionId, LibraryArtifactId,
    MarketplaceRuleId, McpServerId, McpSessionId, McpToolName, PluginId, RuleName, SkillId,
    SkillName,
};
pub use systemprompt_models::bridge::ids::{ManifestSignature, Sha256Digest, ToolPolicy};

macro_rules! bridge_define_token {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(token: impl Into<String>) -> Self {
                Self(token.into())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
            pub fn into_inner(mut self) -> String {
                std::mem::take(&mut self.0)
            }

            #[must_use]
            pub fn redacted(&self) -> String {
                let len = self.0.len();
                if len > 16 {
                    format!("{}...{}", &self.0[..8], &self.0[len - 4..])
                } else {
                    "***".to_owned()
                }
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.redacted())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.redacted())
            }
        }

        impl std::str::FromStr for $name {
            type Err = std::convert::Infallible;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(s.to_owned()))
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_owned())
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                use zeroize::Zeroize;
                self.0.zeroize();
            }
        }
    };
}

bridge_define_token!(PatToken);
bridge_define_token!(BearerToken);
bridge_define_token!(LoopbackSecret);
bridge_define_token!(ProxySecret);
bridge_define_token!(HookToken);
bridge_define_token!(HostToken);
bridge_define_token!(PinnedPubKey);
