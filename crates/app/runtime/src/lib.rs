//! `systemprompt-runtime` — application runtime services.
//!
//! This crate hosts [`AppContext`], the lifecycle [`AppContextBuilder`],
//! the inventory-driven module API and well-known route registries,
//! per-module installation helpers, startup validation, and the typed
//! [`RuntimeError`] / [`RuntimeResult`] error boundary used by all of
//! the above.
//!
//! Public APIs return [`RuntimeResult<T>`]. [`RuntimeError`] composes
//! upstream typed errors (`ConfigError`, `RepositoryError`,
//! `FilesError`, `UserError`, `LoaderError`, `AnalyticsError`,
//! `ProfileBootstrapError`, `PathError`) via `#[from]` and absorbs
//! stringifies still-anyhow upstream calls into [`RuntimeError::Internal`].
//!
//! # Feature flags
//!
//! | Feature       | Effect                                                          |
//! |---------------|------------------------------------------------------------------|
//! | (default)     | Core context, builder, registries, validation                   |
//! | `geolocation` | Enables MaxMind GeoIP2 loading via `maxminddb` and pulls in `systemprompt-analytics/geolocation` |

mod builder;
mod context;
mod context_loaders;
mod context_traits;
mod database_context;
mod error;
mod registry;
mod span;
mod startup_validation;
mod validation;
mod wellknown;

pub use builder::AppContextBuilder;
pub use context::{AppContext, AppContextParts};
pub use database_context::DatabaseContext;
pub use error::{RuntimeError, RuntimeResult};
pub use registry::{ModuleApiRegistration, ModuleApiRegistry, ModuleType, WellKnownRoute};
pub use span::create_request_span;
pub use startup_validation::{
    FilesConfigValidator, StartupValidator, display_validation_report, display_validation_warnings,
};
pub use systemprompt_database::MigrationConfig;
pub use validation::{validate_database_path, validate_system};
pub use wellknown::{WellKnownMetadata, get_wellknown_metadata};

pub use systemprompt_models::modules::ServiceCategory;

#[macro_export]
macro_rules! register_module_api {
    ($module_name:literal, $category:expr, $router_fn:expr, $auth_required:expr, $module_type:expr) => {
        inventory::submit! {
            $crate::ModuleApiRegistration {
                module_name: $module_name,
                category: $category,
                module_type: $module_type,
                router_fn: $router_fn,
                auth_required: $auth_required,
            }
        }
    };
    ($module_name:literal, $category:expr, $router_fn:expr, $auth_required:expr) => {
        inventory::submit! {
            $crate::ModuleApiRegistration {
                module_name: $module_name,
                category: $category,
                module_type: $crate::ModuleType::Regular,
                router_fn: $router_fn,
                auth_required: $auth_required,
            }
        }
    };
}

#[macro_export]
macro_rules! register_wellknown_route {
    ($path:literal, $handler:expr, $methods:expr, name: $name:literal, description: $desc:literal) => {
        inventory::submit! {
            $crate::WellKnownRoute {
                path: $path,
                handler_fn: $handler,
                methods: $methods,
            }
        }

        inventory::submit! {
            $crate::WellKnownMetadata::new($path, $name, $desc)
        }
    };

    ($path:literal, $handler:expr, $methods:expr) => {
        inventory::submit! {
            $crate::WellKnownRoute {
                path: $path,
                handler_fn: $handler,
                methods: $methods,
            }
        }
    };
}
