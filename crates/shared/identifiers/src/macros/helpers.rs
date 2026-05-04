/// Internal helper used by `define_id!` to install the common trait
/// surface (`Display`, `AsRef<str>`, `ToDbValue`, `From<Self> for String`,
/// `PartialEq<&str>`, `Borrow<str>`) on a generated identifier type.
#[doc(hidden)]
#[macro_export]
macro_rules! __define_id_common {
    ($name:ident) => {
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
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

/// Internal helper used by validating `define_id!` variants to install
/// `TryFrom<String>`, `TryFrom<&str>`, `FromStr`, and a validating
/// `Deserialize` impl on a generated identifier type.
#[doc(hidden)]
#[macro_export]
macro_rules! __define_id_validated_conversions {
    ($name:ident) => {
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

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::try_new(s).map_err(serde::de::Error::custom)
            }
        }
    };
}
