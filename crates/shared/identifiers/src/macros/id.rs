#[macro_export]
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

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
            pub fn try_new(value: impl Into<String>) -> Result<Self, $crate::error::IdValidationError> {
                let value = value.into();
                if value.is_empty() {
                    return Err($crate::error::IdValidationError::empty(stringify!($name)));
                }
                Ok(Self(value))
            }

            // Why: panicking convenience constructor for static call sites where the input is known-valid; clippy's expect lint is suppressed because validation failure here is a programmer-bug invariant.
            #[allow(clippy::expect_used)]
            pub fn new(value: impl Into<String>) -> Self {
                Self::try_new(value).expect(concat!(stringify!($name), " cannot be empty"))
            }

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
            pub fn try_new(value: impl Into<String>) -> Result<Self, $crate::error::IdValidationError> {
                let value = value.into();
                let validator: fn(&str) -> Result<(), $crate::error::IdValidationError> = $validator;
                validator(&value)?;
                Ok(Self(value))
            }

            // Why: panicking convenience constructor for static call sites where the input is known-valid.
            #[allow(clippy::expect_used)]
            pub fn new(value: impl Into<String>) -> Self {
                Self::try_new(value).expect(concat!(stringify!($name), " validation failed"))
            }

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
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }
        }
    };

    ($name:ident, system) => {
        $crate::define_id!($name);

        impl $name {
            pub fn system() -> Self {
                Self("system".to_string())
            }
        }
    };

    ($name:ident, generate, system) => {
        $crate::define_id!($name);

        impl $name {
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

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
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

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
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

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
