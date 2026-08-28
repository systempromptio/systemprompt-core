//! Tests for `systemprompt-authz` integration into core.

#![allow(clippy::all)]

#[cfg(test)]
mod access_control_config;
#[cfg(test)]
mod act_chain;
#[cfg(test)]
mod actor_kind_schema;
#[cfg(test)]
mod authz_context;
#[cfg(test)]
mod bootstrap;
#[cfg(test)]
mod config_validate;
#[cfg(test)]
mod db_sink_actor;
#[cfg(test)]
mod decision_schema;
#[cfg(test)]
mod decision_types;
#[cfg(test)]
mod entity_kinds;
#[cfg(test)]
mod entity_row;
#[cfg(test)]
mod governance_audit_repo;
#[cfg(test)]
mod hook_runtime;
#[cfg(test)]
mod hooks_and_composite;
#[cfg(test)]
mod ingestion_db;
#[cfg(test)]
mod ingestion_yaml_path;
#[cfg(test)]
mod marketplace_floor;
#[cfg(test)]
mod parent_chain;
#[cfg(test)]
mod profile_governance;
#[cfg(test)]
mod registered_entities;
#[cfg(test)]
mod registry_discovery;
#[cfg(test)]
mod request_wire_compat;
#[cfg(test)]
mod resolver;
#[cfg(test)]
mod resolver_parents;
#[cfg(test)]
mod rule_based_hook;
#[cfg(test)]
mod subject_attributes;
#[cfg(test)]
mod webhook_hook;
