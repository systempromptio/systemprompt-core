//! Unit tests for the pure parts of `systemprompt_notifications`: the rule
//! evaluator, stream bands, coalescing and digest windows, the default router
//! and publish policy, action prompts, and the member's contributions.

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
mod quiet_hours;
#[cfg(test)]
mod rules;
#[cfg(test)]
mod slot_handles;
