//! Expansion of the `checked` and `uuid` arms of [`define_id!`](crate::define_id):
//! a trusted `new`, a validating `try_new`, validating `Deserialize`/`FromStr`,
//! and no `From<String>`. The `uuid` arm adds `generate`, `from_uuid` and
//! `to_uuid`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[doc(hidden)]
#[macro_export]
macro_rules! __define_id_checked {
    ($name:ident, $validator:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, schemars::JsonSchema)]
        #[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
        #[cfg_attr(feature = "sqlx", sqlx(transparent))]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(id: impl Into<String>) -> Self {
                Self(id.into())
            }

            pub fn try_new(value: impl Into<String>) -> Result<Self, $crate::error::IdValidationError> {
                let value = value.into();
                let validator: fn(&str) -> Result<(), $crate::error::IdValidationError> = $validator;
                validator(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        $crate::__define_id_validated_conversions!($name);
        $crate::__define_id_common!($name);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __define_id_uuid {
    ($name:ident) => {
        $crate::define_id!($name, checked, |value| {
            $crate::macros::validate_uuid(stringify!($name), value)
        });

        impl $name {
            pub fn generate() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            #[must_use]
            pub fn from_uuid(value: uuid::Uuid) -> Self {
                Self(value.to_string())
            }

            pub fn to_uuid(&self) -> Result<uuid::Uuid, $crate::error::IdValidationError> {
                uuid::Uuid::parse_str(&self.0)
                    .map_err(|e| $crate::error::IdValidationError::uuid(stringify!($name), e))
            }
        }
    };
}
