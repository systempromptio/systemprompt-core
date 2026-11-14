//! Capability modules: each pairs a capability's domain crate with its entry
//! crate behind the capability's feature.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "slack")]
#[cfg_attr(docsrs, doc(cfg(feature = "slack")))]
pub mod slack {
    pub use systemprompt_slack::*;
    pub use systemprompt_slack_entry::*;
}

#[cfg(feature = "agent-runs")]
#[cfg_attr(docsrs, doc(cfg(feature = "agent-runs")))]
pub mod agent_runs {
    pub use systemprompt_agent_runs::*;
    pub use systemprompt_agent_runs_entry as entry;
}

#[cfg(feature = "teams")]
#[cfg_attr(docsrs, doc(cfg(feature = "teams")))]
pub mod teams {
    pub use systemprompt_teams::*;
    pub use systemprompt_teams_entry::*;
}

#[cfg(feature = "notifications")]
#[cfg_attr(docsrs, doc(cfg(feature = "notifications")))]
pub mod notifications {
    pub use systemprompt_notifications::*;
    pub use systemprompt_notifications_entry as entry;
}
