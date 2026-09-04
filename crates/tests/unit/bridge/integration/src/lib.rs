#![allow(clippy::all)]

#[cfg(test)]
mod claude_code_cli;
#[cfg(test)]
mod codex_host;
#[cfg(test)]
mod codex_merge;
#[cfg(test)]
mod cowork_artifacts;
#[cfg(test)]
mod doctor_hook_token;
#[cfg(test)]
mod enrol_selection;
#[cfg(test)]
mod gateway_hook_token;
#[cfg(test)]
mod hermes_host;
#[cfg(test)]
mod hermes_merge;
// The module under test is the Linux device-cert keystore; `platform_source`
// resolves to the Keychain branch elsewhere, which answers `NotConfigured`
// rather than reading `SP_BRIDGE_DEVICE_CERT` at all.
#[cfg(test)]
mod codex_install;
#[cfg(test)]
mod host_app_contract;
#[cfg(all(test, target_os = "linux"))]
mod keystore_linux;
#[cfg(test)]
mod managed_skills;
#[cfg(test)]
mod opencode_fallback;
#[cfg(test)]
mod opencode_host;
#[cfg(test)]
mod opencode_merge;
#[cfg(test)]
mod plugin_oauth;
#[cfg(test)]
mod plugin_oauth_store;
#[cfg(test)]
mod profile_state;
#[cfg(test)]
mod profile_state_verdicts;
#[cfg(test)]
mod proxy_probe;
#[cfg(test)]
mod reapply;
#[cfg(test)]
mod reapply_decision;
#[cfg(test)]
mod reg_profile;
