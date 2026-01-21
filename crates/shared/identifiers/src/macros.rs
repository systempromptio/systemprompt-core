#[macro_export]
macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
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

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl From<&$name> for String {
            fn from(id: &$name) -> Self {
                id.0.clone()
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        impl PartialEq<$name> for str {
            fn eq(&self, other: &$name) -> bool {
                self == other.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
    };

    ($name:ident, non_empty) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
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

            #[allow(clippy::expect_used)]
            pub fn new(value: impl Into<String>) -> Self {
                Self::try_new(value).expect(concat!(stringify!($name), " cannot be empty"))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = $crate::error::IdValidationError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::try_new(s)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = $crate::error::IdValidationError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                Self::try_new(s)
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::error::IdValidationError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_new(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::try_new(s).map_err(serde::de::Error::custom)
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

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl From<&$name> for String {
            fn from(id: &$name) -> Self {
                id.0.clone()
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        impl PartialEq<$name> for str {
            fn eq(&self, other: &$name) -> bool {
                self == other.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
    };

    ($name:ident, validated, $validator:expr) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
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

            #[allow(clippy::expect_used)]
            pub fn new(value: impl Into<String>) -> Self {
                Self::try_new(value).expect(concat!(stringify!($name), " validation failed"))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = $crate::error::IdValidationError;

            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::try_new(s)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = $crate::error::IdValidationError;

            fn try_from(s: &str) -> Result<Self, Self::Error> {
                Self::try_new(s)
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::error::IdValidationError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_new(s)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::try_new(s).map_err(serde::de::Error::custom)
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

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl From<&$name> for String {
            fn from(id: &$name) -> Self {
                id.0.clone()
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        impl PartialEq<$name> for str {
            fn eq(&self, other: &$name) -> bool {
                self == other.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
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
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
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

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl From<&$name> for String {
            fn from(id: &$name) -> Self {
                id.0.clone()
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        impl PartialEq<$name> for str {
            fn eq(&self, other: &$name) -> bool {
                self == other.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
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
        #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
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

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl From<&$name> for String {
            fn from(id: &$name) -> Self {
                id.0.clone()
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<$name> for &str {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        impl PartialEq<$name> for str {
            fn eq(&self, other: &$name) -> bool {
                self == other.0
            }
        }

        impl std::borrow::Borrow<str> for $name {
            fn borrow(&self) -> &str {
                &self.0
            }
        }
    };
}

pub use define_id;
