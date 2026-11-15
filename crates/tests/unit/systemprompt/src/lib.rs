//! Composition tests for the facade: reference order, install refusals,
//! provider standing, disabled members, kernel handles, the CLI re-check of
//! a forged command handoff and the built-in gateway prefixes.

#[cfg(test)]
mod cli_probe;
#[cfg(test)]
mod command_disabled;
#[cfg(test)]
mod composition_order;
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
