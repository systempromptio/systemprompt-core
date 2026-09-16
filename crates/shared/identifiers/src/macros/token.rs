//! `define_token!` macro for secret-bearing identifiers.
//!
//! Both `Debug` and `Display` render the redacted form, so a token that ends
//! up in a `tracing` field or a derived error `Debug` never prints the
//! credential; `as_str` is the only way to read it back.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[macro_export]
macro_rules! define_token {
    ($name:ident) => {
        #[derive(
            Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
        )]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(token: impl Into<String>) -> Self {
                Self(token.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            #[must_use]
            pub fn redacted(&self) -> String {
                let chars = self.0.chars().count();
                if chars <= 16 {
                    return "*".repeat(chars.min(8));
                }
                let head: String = self.0.chars().take(8).collect();
                let tail: String = self.0.chars().skip(chars - 4).collect();
                format!("{head}...{tail}")
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.redacted())
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&self.redacted())
                    .finish()
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl $crate::ToDbValue for $name {
            fn to_db_value(&self) -> $crate::DbValue {
                $crate::DbValue::String(self.0.clone())
            }
        }

        impl $crate::ToDbValue for &$name {
            fn to_db_value(&self) -> $crate::DbValue {
                $crate::DbValue::String(self.0.clone())
            }
        }
    };
}
