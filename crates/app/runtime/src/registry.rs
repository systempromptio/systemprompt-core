//! Module API and well-known route registries built from `inventory`.
//!
//! Modules register HTTP routers with
//! [`register_module_api!`](crate::register_module_api) and well-known
//! endpoints with
//! [`register_wellknown_route!`](crate::register_wellknown_route). Both submit
//! to `inventory` collectors that this module materialises into runtime maps.

use axum::Router;
use std::collections::HashMap;

pub use systemprompt_models::modules::{Module, ModuleType, Modules, ServiceCategory};

use crate::AppContext;

/// Runtime map of module name -> registered router and metadata.
#[derive(Debug)]
pub struct ModuleApiRegistry {
    registry: HashMap<String, ModuleApiImpl>,
}

#[derive(Debug)]
struct ModuleApiImpl {
    category: ServiceCategory,
    module_type: ModuleType,
    router_fn: fn(&AppContext) -> Router,
    auth_required: bool,
}

/// `inventory` registration record submitted by
/// [`register_module_api!`](crate::register_module_api).
#[derive(Debug, Copy, Clone)]
pub struct ModuleApiRegistration {
    /// Module name (matches the manifest key).
    pub module_name: &'static str,
    /// Service category bucket.
    pub category: ServiceCategory,
    /// Whether the module is regular or system-only.
    pub module_type: ModuleType,
    /// Function that builds the module's `axum::Router`.
    pub router_fn: fn(&AppContext) -> Router,
    /// `true` when requests must be authenticated.
    pub auth_required: bool,
}

inventory::collect!(ModuleApiRegistration);

/// `inventory` registration record submitted by
/// [`register_wellknown_route!`](crate::register_wellknown_route).
#[derive(Debug, Clone, Copy)]
pub struct WellKnownRoute {
    /// Path under `/.well-known/`.
    pub path: &'static str,
    /// Function that builds the route's `axum::Router`.
    pub handler_fn: fn(&AppContext) -> Router,
    /// HTTP methods accepted by the route.
    pub methods: &'static [axum::http::Method],
}

inventory::collect!(WellKnownRoute);

impl Default for ModuleApiRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleApiRegistry {
    /// Build the registry from all submitted [`ModuleApiRegistration`]s.
    pub fn new() -> Self {
        let mut registry = HashMap::new();

        for registration in inventory::iter::<ModuleApiRegistration> {
            let api_impl = ModuleApiImpl {
                category: registration.category,
                module_type: registration.module_type,
                router_fn: registration.router_fn,
                auth_required: registration.auth_required,
            };
            registry.insert(registration.module_name.to_string(), api_impl);
        }

        Self { registry }
    }

    /// Materialise the registered router for `module_name`.
    pub fn get_routes(&self, module_name: &str, ctx: &AppContext) -> Option<Router> {
        self.registry
            .get(module_name)
            .map(|impl_| (impl_.router_fn)(ctx))
    }

    /// Service category for `module_name`.
    pub fn get_category(&self, module_name: &str) -> Option<ServiceCategory> {
        self.registry.get(module_name).map(|impl_| impl_.category)
    }

    /// Module type for `module_name`.
    pub fn get_module_type(&self, module_name: &str) -> Option<ModuleType> {
        self.registry
            .get(module_name)
            .map(|impl_| impl_.module_type)
    }

    /// Whether `module_name` requires authenticated requests.
    pub fn get_auth_required(&self, module_name: &str) -> Option<bool> {
        self.registry
            .get(module_name)
            .map(|impl_| impl_.auth_required)
    }

    /// All module names registered under `category`.
    pub fn modules_by_category(&self, category: ServiceCategory) -> Vec<String> {
        self.registry
            .iter()
            .filter(|(_, impl_)| matches!(impl_.category, c if c as u8 == category as u8))
            .map(|(name, _)| name.clone())
            .collect()
    }
}

/// Per-module router-resolution helpers.
pub trait ModuleRuntime {
    /// Resolve the API router for `self` given `ctx` and `registry`.
    fn routes(&self, ctx: &AppContext, registry: &ModuleApiRegistry) -> Option<Router>;
    /// Build a fresh [`ModuleApiRegistry`] from current `inventory` state.
    fn create_api_registry(&self) -> ModuleApiRegistry;
}

impl ModuleRuntime for Module {
    fn routes(&self, ctx: &AppContext, registry: &ModuleApiRegistry) -> Option<Router> {
        if let Some(api) = &self.api {
            if api.enabled {
                return registry.get_routes(&self.name, ctx);
            }
        }
        None
    }

    fn create_api_registry(&self) -> ModuleApiRegistry {
        ModuleApiRegistry::new()
    }
}

impl ModuleRuntime for Modules {
    fn routes(&self, _ctx: &AppContext, _registry: &ModuleApiRegistry) -> Option<Router> {
        None
    }

    fn create_api_registry(&self) -> ModuleApiRegistry {
        ModuleApiRegistry::new()
    }
}
