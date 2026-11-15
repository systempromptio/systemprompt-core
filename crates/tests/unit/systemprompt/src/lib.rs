//! Composition tests for the facade: reference order, install refusals,
//! provider standing, disabled members, kernel handles, the CLI re-check of
//! a forged command handoff, the built-in gateway prefixes, and the
//! analytics-http and content-http members as the reference composition
//! admits them, and the brain and notifications capability slots and routes.

#[cfg(test)]
mod analytics_http;
#[cfg(test)]
mod capability_slots;
#[cfg(test)]
mod cli_probe;
#[cfg(test)]
mod command_disabled;
#[cfg(test)]
mod composition_order;
#[cfg(test)]
mod content_http;
#[cfg(test)]
mod core_ids;
#[cfg(test)]
mod disable_probe;
#[cfg(test)]
mod forged_commands;
#[cfg(test)]
mod gateway_prefixes;
#[cfg(test)]
mod handed;
#[cfg(test)]
mod install_refusals;
#[cfg(test)]
mod probe;
#[cfg(test)]
mod providers;
