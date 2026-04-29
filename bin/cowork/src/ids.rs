use std::fmt;

#[derive(Debug, thiserror::Error)]
pub enum IdValidationError {
    #[error("{type_name} cannot be empty")]
    Empty { type_name: &'static str },
    #[error("{type_name} is invalid: {reason}")]
    Invalid {
        type_name: &'static str,
        reason: String,
    },
}

impl IdValidationError {
    pub fn empty(type_name: &'static str) -> Self {
        Self::Empty { type_name }
    }

    pub fn invalid(type_name: &'static str, reason: impl Into<String>) -> Self {
        Self::Invalid {
            type_name,
            reason: reason.into(),
        }
    }
}

#[macro_export]
macro_rules! cowork_define_id {
    ($name:ident) => {
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash,
            serde::Serialize, serde::Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
            pub fn into_inner(self) -> String { self.0 }
        }

        impl From<String> for $name { fn from(s: String) -> Self { Self(s) } }
        impl From<&str> for $name { fn from(s: &str) -> Self { Self(s.to_string()) } }

        $crate::cowork_id_common!($name);
    };

    ($name:ident, non_empty) => {
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(
                value: impl Into<String>,
            ) -> Result<Self, $crate::ids::IdValidationError> {
                let value = value.into();
                if value.is_empty() {
                    return Err($crate::ids::IdValidationError::empty(stringify!($name)));
                }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str { &self.0 }
            pub fn into_inner(self) -> String { self.0 }
        }

        $crate::cowork_id_validated_conversions!($name);
        $crate::cowork_id_common!($name);
    };

    ($name:ident, validated, $validator:expr) => {
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn try_new(
                value: impl Into<String>,
            ) -> Result<Self, $crate::ids::IdValidationError> {
                let value = value.into();
                let validator: fn(&str) -> Result<(), $crate::ids::IdValidationError> =
                    $validator;
                validator(&value)?;
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str { &self.0 }
            pub fn into_inner(self) -> String { self.0 }
        }

        $crate::cowork_id_validated_conversions!($name);
        $crate::cowork_id_common!($name);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! cowork_id_common {
    ($name:ident) => {
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { &self.0 }
        }
        impl From<$name> for String {
            fn from(id: $name) -> Self { id.0 }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! cowork_id_validated_conversions {
    ($name:ident) => {
        impl TryFrom<String> for $name {
            type Error = $crate::ids::IdValidationError;
            fn try_from(s: String) -> Result<Self, Self::Error> { Self::try_new(s) }
        }
        impl TryFrom<&str> for $name {
            type Error = $crate::ids::IdValidationError;
            fn try_from(s: &str) -> Result<Self, Self::Error> { Self::try_new(s) }
        }
        impl std::str::FromStr for $name {
            type Err = $crate::ids::IdValidationError;
            fn from_str(s: &str) -> Result<Self, Self::Err> { Self::try_new(s) }
        }
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where D: serde::Deserializer<'de> {
                let s = String::deserialize(deserializer)?;
                Self::try_new(s).map_err(serde::de::Error::custom)
            }
        }
    };
}

#[macro_export]
macro_rules! cowork_define_token {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(token: impl Into<String>) -> Self { Self(token.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
            pub fn into_inner(mut self) -> String { std::mem::take(&mut self.0) }

            #[must_use]
            pub fn redacted(&self) -> String {
                let len = self.0.len();
                if len > 16 {
                    format!("{}...{}", &self.0[..8], &self.0[len - 4..])
                } else {
                    "***".to_string()
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
            fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self(s.to_string())) }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str { &self.0 }
        }

        impl From<String> for $name { fn from(s: String) -> Self { Self(s) } }
        impl From<&str> for $name { fn from(s: &str) -> Self { Self(s.to_string()) } }

        impl Drop for $name {
            fn drop(&mut self) {
                use zeroize::Zeroize;
                self.0.zeroize();
            }
        }
    };
}

cowork_define_token!(PatToken);
cowork_define_token!(BearerToken);
cowork_define_token!(LoopbackSecret);
cowork_define_token!(ProxySecret);
cowork_define_token!(ManifestSignature);
cowork_define_token!(PinnedPubKey);

cowork_define_id!(PluginId, non_empty);
cowork_define_id!(SkillId, non_empty);
cowork_define_id!(SkillName, non_empty);
cowork_define_id!(ManagedMcpServerName, non_empty);
cowork_define_id!(ToolName, non_empty);
cowork_define_id!(PrefsDomain, non_empty);
cowork_define_id!(PrefsKey, non_empty);
cowork_define_id!(ModelId, non_empty);
cowork_define_id!(KeystoreRef, non_empty);
cowork_define_id!(CertFingerprint, non_empty);
cowork_define_id!(QueryKey, non_empty);

cowork_define_id!(PrefsValue);
cowork_define_id!(QueryValue);

cowork_define_id!(Sha256Digest, validated, |value: &str| {
    if value.len() != 64 {
        return Err(IdValidationError::invalid(
            "Sha256Digest",
            format!("expected 64 hex chars, got {}", value.len()),
        ));
    }
    if !value.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err(IdValidationError::invalid(
            "Sha256Digest",
            "expected lowercase hex characters",
        ));
    }
    Ok(())
});

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolPolicy {
    Allow,
    Deny,
    Prompt,
}

impl fmt::Display for ToolPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Allow => f.write_str("allow"),
            Self::Deny => f.write_str("deny"),
            Self::Prompt => f.write_str("prompt"),
        }
    }
}
