//! Outcome signals: what happened after facts were injected.
//!
//! [`cited`] finds the injected fact ids an assistant response names, and
//! [`OutcomeService::observe_response`] records one `cited_in_response`
//! outcome per cited fact, then asks every extension [`OutcomeSignal`]
//! for its own. [`corrected`] records `user_corrected_fact` against every
//! recall that injected a fact a person disputed, retracted or edited
//! within the profile's correction window. Every outcome is unique per
//! recall, fact and signal, so a replayed observation writes nothing.
//!
//! [`OutcomeSignal`]: systemprompt_extension::brain::OutcomeSignal
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod cited;
pub mod corrected;
mod service;

pub use cited::cited_facts;
pub use corrected::on_correction;
pub use service::OutcomeService;
