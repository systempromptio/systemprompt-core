//! Declarative `domain_error!` macro for domain crates.
//!
//! Eliminates the boilerplate `Database`, `Io`, `Json`, `Validation`,
//! `NotFound`, etc. variants that every crate hand-rolls. Domain crates
//! invoke the macro with a `common: [...]` list of pre-canned variants and
//! their own domain-specific variants underneath.
//!
//! ```ignore
//! use systemprompt_models::domain_error;
//!
//! domain_error! {
//!     pub enum FilesError {
//!         common: [repository, io, json, validation, not_found],
//!
//!         #[error("permission denied: {path}")]
//!         PermissionDenied { path: String },
//!     }
//! }
//! ```
//!
//! The `repository` token funnels database errors through the canonical
//! `systemprompt_database::RepositoryError` rather than `sqlx::Error`
//! directly, so the layer boundary is preserved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[macro_export]
macro_rules! domain_error {
    (
        $(#[$emeta:meta])*
        pub enum $name:ident {
            common: [ $($common:ident),* $(,)? ]
            $(, $($body:tt)*)?
        }
    ) => {
        $crate::__domain_error_emit! {
            @attrs [$(#[$emeta])*]
            @name $name
            @commons [ $($common)* ]
            @body { $($($body)*)? }
        }
    };
    (
        $(#[$emeta:meta])*
        pub enum $name:ident {
            $($body:tt)*
        }
    ) => {
        $(#[$emeta])*
        #[derive(::std::fmt::Debug, ::thiserror::Error)]
        pub enum $name {
            $($body)*
        }
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! __domain_error_emit {
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [io $($rest:ident)*] @body { $($body:tt)* }) => {
        $crate::__domain_error_emit! {
            @attrs [$($attrs)*]
            @name $name
            @commons [$($rest)*]
            @body {
                #[error("io: {0}")]
                Io(#[from] ::std::io::Error),
                $($body)*
            }
        }
    };
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [json $($rest:ident)*] @body { $($body:tt)* }) => {
        $crate::__domain_error_emit! {
            @attrs [$($attrs)*]
            @name $name
            @commons [$($rest)*]
            @body {
                #[error("json: {0}")]
                Json(#[from] ::serde_json::Error),
                $($body)*
            }
        }
    };
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [yaml $($rest:ident)*] @body { $($body:tt)* }) => {
        $crate::__domain_error_emit! {
            @attrs [$($attrs)*]
            @name $name
            @commons [$($rest)*]
            @body {
                #[error("yaml: {0}")]
                Yaml(#[from] ::serde_yaml::Error),
                $($body)*
            }
        }
    };
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [validation $($rest:ident)*] @body { $($body:tt)* }) => {
        $crate::__domain_error_emit! {
            @attrs [$($attrs)*]
            @name $name
            @commons [$($rest)*]
            @body {
                #[error("validation: {0}")]
                Validation(::std::string::String),
                $($body)*
            }
        }
    };
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [not_found $($rest:ident)*] @body { $($body:tt)* }) => {
        $crate::__domain_error_emit! {
            @attrs [$($attrs)*]
            @name $name
            @commons [$($rest)*]
            @body {
                #[error("not found: {0}")]
                NotFound(::std::string::String),
                $($body)*
            }
        }
    };
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [config $($rest:ident)*] @body { $($body:tt)* }) => {
        $crate::__domain_error_emit! {
            @attrs [$($attrs)*]
            @name $name
            @commons [$($rest)*]
            @body {
                #[error("config: {0}")]
                Config(::std::string::String),
                $($body)*
            }
        }
    };
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [http $($rest:ident)*] @body { $($body:tt)* }) => {
        $crate::__domain_error_emit! {
            @attrs [$($attrs)*]
            @name $name
            @commons [$($rest)*]
            @body {
                #[error("http: {0}")]
                Http(#[from] ::reqwest::Error),
                $($body)*
            }
        }
    };
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [repository $($rest:ident)*] @body { $($body:tt)* }) => {
        $crate::__domain_error_emit! {
            @attrs [$($attrs)*]
            @name $name
            @commons [$($rest)*]
            @body {
                #[error("repository: {0}")]
                Repository(#[from] ::systemprompt_database::RepositoryError),
                $($body)*
            }
        }
    };
    (@attrs [$($attrs:tt)*] @name $name:ident @commons [] @body { $($body:tt)* }) => {
        $($attrs)*
        #[derive(::std::fmt::Debug, ::thiserror::Error)]
        pub enum $name {
            $($body)*
        }
    };
}
