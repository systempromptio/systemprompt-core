//! Tests for `systemprompt-marketplace-entry`: the marketplace route set
//! materialized over a fixture context's kernel state — the catalogue
//! renders, the confined manifest read, the plugin-file misses and the
//! services-watch reads — and the `core marketplace`, `core plugins` and
//! `core skills` commands served through a member host holding the
//! services-tree grant, including the refusal without it and the managed
//! skills the owner-bound organization resolver serves.

#[cfg(test)]
mod common;
#[cfg(test)]
mod core_commands_fs;
#[cfg(test)]
mod core_managed_skills_db;
#[cfg(test)]
mod core_marketplace_explain_fs;
#[cfg(test)]
mod core_marketplace_import_fs;
#[cfg(test)]
mod core_plugins_fs;
#[cfg(test)]
mod core_plugins_list_fs;
#[cfg(test)]
mod core_skills_plugins_fs;
#[cfg(test)]
mod marketplace_yaml;
#[cfg(test)]
mod routes_marketplace;
#[cfg(test)]
mod routes_marketplace_services_watch;
#[cfg(test)]
mod routes_marketplace_success;
