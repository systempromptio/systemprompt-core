mod context;
mod database_context;
mod installation;
mod registry;
mod span;
mod startup_validation;
mod validation;
mod wellknown;

pub use context::{AppContext, AppContextBuilder};
pub use database_context::DatabaseContext;
pub use installation::{install_module, install_module_with_db};
pub use registry::{ModuleApiRegistration, ModuleApiRegistry, ModuleRuntime, WellKnownRoute};
pub use span::create_request_span;
pub use startup_validation::{
    display_validation_report, display_validation_warnings, FilesConfigValidator, StartupValidator,
};
pub use validation::validate_system;
pub use wellknown::{get_wellknown_metadata, WellKnownMetadata};

pub use systemprompt_models::modules::{
    ApiConfig, Module, ModuleDefinition, ModulePermission, ModuleSchema, ModuleSeed, ModuleType,
    Modules, ServiceCategory,
};

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
