#![allow(clippy::all)]

#[cfg(test)]
mod claude_code_cli;
#[cfg(test)]
mod claude_code_cli_dependencies;
#[cfg(test)]
mod codex_foreign_shape;
#[cfg(test)]
mod codex_host;
#[cfg(test)]
mod codex_install;
#[cfg(test)]
mod codex_merge;
#[cfg(test)]
mod cowork_artifacts;
#[cfg(test)]
mod doctor_hook_token;
#[cfg(test)]
mod enrol_claude_code;
#[cfg(test)]
mod enrol_hosts;
#[cfg(test)]
mod enrol_report;
#[cfg(test)]
mod enrol_selection;
#[cfg(test)]
mod gateway_hook_token;
#[cfg(test)]
mod generated_profile_privacy;
#[cfg(test)]
mod hermes_host;
#[cfg(test)]
mod hermes_merge;
#[cfg(test)]
mod host_app_contract;
#[cfg(test)]
mod managed_skills;
#[cfg(test)]
mod node_deps;
#[cfg(test)]
mod opencode_default_model;
#[cfg(test)]
mod opencode_fallback;
#[cfg(test)]
mod opencode_host;
#[cfg(test)]
mod opencode_merge;
#[cfg(all(test, unix))]
mod opencode_read_only_admin_tier;
#[cfg(test)]
mod plugin_oauth;
#[cfg(test)]
mod plugin_oauth_gateway_identity;
#[cfg(test)]
mod plugin_oauth_store;
#[cfg(test)]
mod profile_state;
#[cfg(test)]
mod profile_state_verdicts;
#[cfg(test)]
mod proxy_probe;
#[cfg(test)]
mod purge_state;
#[cfg(test)]
mod reapply;
#[cfg(test)]
mod reapply_decision;
#[cfg(test)]
mod reg_profile;
#[cfg(test)]
mod start_menu_cache;
