/// Declares an opaque secret-token newtype that redacts its value when
/// formatted via `Display`.
///
/// The generated type implements `Debug`, `Clone`, `Eq`, `Hash`, `serde`
/// (transparent), optional `sqlx::Type`, `AsRef<str>`, `ToDbValue`, and
/// `From<String>`/`From<&str>`. `Display` shows a redacted form
/// (`"abcdefgh...wxyz"` for tokens longer than 16 chars, otherwise asterisks).
///
/// Tokens carry the same wire encoding as plain `String` identifiers but make
/// accidental logging safe by default — `tracing::info!("{}", token)` never
/// leaks the secret.
#[macro_export]
macro_rules! define_token {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wraps a raw token string in the typed wrapper.
            pub fn new(token: impl Into<String>) -> Self {
                Self(token.into())
            }

            /// Returns the raw token value. Avoid logging the result directly —
            /// prefer `Display` (`"{}"`), which redacts.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Returns a redacted form safe for logging
            /// (`"abcdefgh...wxyz"` for tokens longer than 16 characters).
            #[must_use]
            pub fn redacted(&self) -> String {
                let len = self.0.len();
                if len <= 16 {
                    "*".repeat(len.min(8))
                } else {
                    format!("{}...{}", &self.0[..8], &self.0[len - 4..])
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.redacted())
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
