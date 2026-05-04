/// Declares a typed string-newtype identifier.
///
/// Generated impls: `Debug`, `Clone`, `Eq`, `Hash`, `Display`, `serde`,
/// optional `sqlx::Type`, and `ToDbValue` interop.
///
/// Variants:
///
/// | Form | Semantics |
/// |------|-----------|
/// | `define_id!(Name)` | Plain newtype. `new` accepts any `Into<String>` and never fails. |
/// | `define_id!(Name, non_empty)` | `try_new` rejects empty strings; `new` panics on empty. |
/// | `define_id!(Name, validated, validator_fn)` | `try_new` runs `validator_fn(&str) -> Result<(), IdValidationError>`. |
/// | `define_id!(Name, generate)` | Adds `Name::generate()` returning a fresh UUID-backed value. |
/// | `define_id!(Name, system)` | Adds `Name::system()` returning the literal `"system"`. |
/// | `define_id!(Name, generate, system)` | Both `generate()` and `system()` constructors. |
/// | `define_id!(Name, schema)` | Plain newtype that also derives `schemars::JsonSchema`. |
/// | `define_id!(Name, generate, schema)` | Schema-derived newtype with `generate()`. |
///
/// All variants implement `ToDbValue`/`AsRef<str>`/`Display`, plus
/// `From<String>`, `From<&str>` for unvalidated forms (and
/// `TryFrom`/`FromStr`/`Deserialize` for validated forms).
#[macro_export]
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wraps a string value as this typed identifier without validation.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// Returns the inner string value.
            pub fn as_str(&self) -> &str {
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
                Self(s.to_string())
            }
        }

        $crate::__define_id_common!($name);
    };

    ($name:ident, non_empty) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Validates and constructs the identifier, rejecting empty strings.
            pub fn try_new(value: impl Into<String>) -> Result<Self, $crate::error::IdValidationError> {
                let value = value.into();
                if value.is_empty() {
                    return Err($crate::error::IdValidationError::empty(stringify!($name)));
                }
                Ok(Self(value))
            }

            /// Constructs the identifier, panicking on validation failure.
            ///
            /// Prefer `try_new` for any value not known at compile time.
            // Why: panicking convenience constructor for static call sites where
            // the input is known-valid; clippy's expect lint is suppressed
            // because validation failure here is a programmer-bug invariant.
            #[allow(clippy::expect_used)]
            pub fn new(value: impl Into<String>) -> Self {
                Self::try_new(value).expect(concat!(stringify!($name), " cannot be empty"))
            }

            /// Returns the inner string value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        $crate::__define_id_validated_conversions!($name);
        $crate::__define_id_common!($name);
    };

    ($name:ident, validated, $validator:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Validates and constructs the identifier using the supplied validator.
            pub fn try_new(value: impl Into<String>) -> Result<Self, $crate::error::IdValidationError> {
                let value = value.into();
                let validator: fn(&str) -> Result<(), $crate::error::IdValidationError> = $validator;
                validator(&value)?;
                Ok(Self(value))
            }

            /// Constructs the identifier, panicking on validation failure.
            // Why: panicking convenience constructor for static call sites
            // where the input is known-valid.
            #[allow(clippy::expect_used)]
            pub fn new(value: impl Into<String>) -> Self {
                Self::try_new(value).expect(concat!(stringify!($name), " validation failed"))
            }

            /// Returns the inner string value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        $crate::__define_id_validated_conversions!($name);
        $crate::__define_id_common!($name);
    };

    ($name:ident, generate) => {
        $crate::define_id!($name);

        impl $name {
            /// Mints a fresh identifier backed by a v4 UUID.
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }
        }
    };

    ($name:ident, system) => {
        $crate::define_id!($name);

        impl $name {
            /// Returns the canonical `"system"` identifier.
            pub fn system() -> Self {
                Self("system".to_string())
            }
        }
    };

    ($name:ident, generate, system) => {
        $crate::define_id!($name);

        impl $name {
            /// Mints a fresh identifier backed by a v4 UUID.
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            /// Returns the canonical `"system"` identifier.
            pub fn system() -> Self {
                Self("system".to_string())
            }
        }
    };

    ($name:ident, schema) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wraps a string value as this typed identifier without validation.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// Returns the inner string value.
            pub fn as_str(&self) -> &str {
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
                Self(s.to_string())
            }
        }

        $crate::__define_id_common!($name);
    };

    ($name:ident, generate, schema) => {
        $crate::define_id!(@ $name, schema);

        impl $name {
            /// Mints a fresh identifier backed by a v4 UUID.
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }
        }
    };

    (@ $name:ident, schema) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Wraps a string value as this typed identifier without validation.
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            /// Returns the inner string value.
            pub fn as_str(&self) -> &str {
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
                Self(s.to_string())
            }
        }

        $crate::__define_id_common!($name);
    };
}
