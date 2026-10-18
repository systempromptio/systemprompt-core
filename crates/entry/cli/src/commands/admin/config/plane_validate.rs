//! Offline parse of every registered config plane's declaration, run by
//! `admin config validate`.
//!
//! No database is touched: each plane's `load_declared` reads only the
//! services tree. An absent declaration is valid (the plane is then
//! database-managed); an unreadable or invalid one, a duplicate plane name and
//! a `config_planes.authority` key no extension registers are not.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::PathBuf;

use systemprompt_extension::ExtensionRegistry;
use systemprompt_manifest::Config;
use systemprompt_runtime::ConfigPlaneSet;

use super::types::ConfigFileInfo;

pub fn append_full_validation(target: Option<&String>, results: &mut Vec<ConfigFileInfo>) -> bool {
    if target.is_some() {
        return true;
    }
    let planes = plane_declaration_results();
    let valid = planes.iter().all(|p| p.valid);
    results.extend(planes);
    valid
}

pub fn plane_declaration_results() -> Vec<ConfigFileInfo> {
    let config = match Config::get() {
        Ok(config) => config,
        Err(error) => return vec![failure("config_planes", &error.to_string())],
    };
    let registry = match ExtensionRegistry::discover() {
        Ok(registry) => registry,
        Err(error) => return vec![failure("config_planes", &error.to_string())],
    };
    let set = match ConfigPlaneSet::new(
        registry.all_config_planes(),
        config.planes.authority.clone(),
        PathBuf::from(&config.services_path),
    ) {
        Ok(set) => set,
        Err(error) => return vec![failure("config_planes", &error.to_string())],
    };
    let mut results: Vec<ConfigFileInfo> = set
        .planes()
        .iter()
        .map(|plane| {
            let path = set.services_root().join(plane.declared_path());
            let error = plane
                .load_declared(set.services_root())
                .err()
                .map(|e| e.to_string());
            ConfigFileInfo {
                path: path.display().to_string(),
                section: format!("plane:{}", plane.name()),
                exists: path.exists(),
                valid: error.is_none(),
                error,
            }
        })
        .collect();
    for key in set.unregistered_authority_keys() {
        results.push(failure(
            "profile",
            &format!("config_planes.authority names '{key}', which no extension registers"),
        ));
    }
    results
}

fn failure(section: &str, message: &str) -> ConfigFileInfo {
    ConfigFileInfo {
        path: "config_planes".to_owned(),
        section: section.to_owned(),
        exists: true,
        valid: false,
        error: Some(message.to_owned()),
    }
}
