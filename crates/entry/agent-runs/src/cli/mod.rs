//! The agent-runs capability's CLI contributions: the `agent_run_origins`
//! seam `plugins seams` lists, owned by the `agent_runs` extension and
//! enumerated from the run origins this binary registers.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_cli::kernel::{LinkedSeamRegistration, named};

systemprompt_cli::register_linked_seam!(LinkedSeamRegistration {
    name: "agent_run_origins",
    owner: "systemprompt-agent-runs",
    extension: Some("agent_runs"),
    entries: || named(systemprompt_agent_runs::origin::registered()),
});
