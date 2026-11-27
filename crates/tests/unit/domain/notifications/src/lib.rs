//! Unit tests for the pure parts of `systemprompt_notifications`: the rule
//! evaluator, stream bands, coalescing and digest windows, the default router
//! and publish policy, action prompts, the member's contributions, and the push
//! channels driven through stub providers.

#[cfg(test)]
mod action;
#[cfg(test)]
mod coalesce;
#[cfg(test)]
mod defaults;
#[cfg(test)]
mod failure_stage;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod push_channel;
#[cfg(test)]
mod push_declaration;
#[cfg(test)]
mod push_support;
#[cfg(test)]
mod quiet_hours;
#[cfg(test)]
mod rules;
#[cfg(test)]
mod slot_handles;
