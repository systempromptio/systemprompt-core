mod api_paths;
mod cli_paths;
mod service_category;
mod types;

pub use api_paths::ApiPaths;
pub use cli_paths::CliPaths;
pub use service_category::ServiceCategory;
pub use types::{
    ApiConfig, Module, ModuleDefinition, ModulePermission, ModuleSchema, ModuleSeed, ModuleType,
    SchemaSource, SeedSource,
};

use anyhow::{bail, Result};

#[derive(Clone, Debug)]
pub struct Modules {
    modules: Vec<Module>,
}

impl Modules {
    pub fn from_vec(modules: Vec<Module>) -> Result<Self> {
        let modules = Self::resolve_dependencies(modules)?;
        Ok(Self { modules })
    }

    pub const fn all(&self) -> &Vec<Module> {
        &self.modules
    }

    pub fn get(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.name == name)
    }

    pub fn resolve_dependencies(mut modules: Vec<Module>) -> Result<Vec<Module>> {
        use std::collections::HashSet;

        let mut ordered = Vec::new();
        let mut processed = HashSet::new();
        let all_module_names: HashSet<String> = modules.iter().map(|m| m.name.clone()).collect();

        while !modules.is_empty() {
            let to_process: Vec<_> = modules
                .iter()
                .filter(|m| {
                    m.dependencies
                        .iter()
                        .all(|dep| processed.contains(dep.as_str()))
                })
                .cloned()
                .collect();

            if to_process.is_empty() && !modules.is_empty() {
                let missing_deps: Vec<_> = modules
                    .iter()
                    .flat_map(|m| {
                        m.dependencies
                            .iter()
                            .filter(|dep| {
                                !all_module_names.contains(*dep)
                                    && !processed.contains(dep.as_str())
                            })
                            .map(move |dep| (m.name.clone(), dep.clone()))
                    })
                    .collect();

                if !missing_deps.is_empty() {
                    let missing_list: Vec<_> = missing_deps
                        .iter()
                        .map(|(m, d)| format!("{m} -> {d}"))
                        .collect();
                    bail!("Missing module dependencies: {}", missing_list.join(", "));
                }

                let remaining: Vec<_> = modules.iter().map(|m| m.name.clone()).collect();
                bail!("Circular dependency detected in modules: {remaining:?}");
            }

            for module in &to_process {
                ordered.push(module.clone());
                processed.insert(module.name.clone());
            }

            modules.retain(|module| !processed.contains(module.name.as_str()));
        }

        Ok(ordered)
    }

    pub fn list_names(&self) -> Vec<String> {
        self.modules.iter().map(|m| m.name.clone()).collect()
    }

    pub fn get_provided_audiences() -> Vec<String> {
        vec!["a2a".to_string(), "api".to_string(), "mcp".to_string()]
    }

    pub fn get_valid_audiences(&self, module_name: &str) -> Vec<String> {
        self.get(module_name)
            .map_or_else(Self::get_provided_audiences, |module| {
                module.audience.clone()
            })
    }

    pub fn get_server_audiences(_server_name: &str, _port: u16) -> Vec<String> {
        Self::get_provided_audiences()
    }
}
