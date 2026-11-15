//! The read-only gate for `cloud fleet run`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};

use systemprompt_cli::kernel::{RoutingClass, describe_command_line};

pub fn ensure_read_only(command: &[String]) -> Result<()> {
    let descriptor = describe_command_line(command)?;
    if descriptor.cloud_operator() {
        bail!(
            "cloud commands run on this machine, not on tenants; fleet run forwards tenant \
             commands"
        );
    }
    if descriptor.data_impact().destroys_data()
        || descriptor.routing_class() != RoutingClass::ReadOnly
    {
        bail!(
            "'{}' is not a read-only command; cloud fleet run forwards read-only commands only",
            command.join(" ")
        );
    }
    Ok(())
}
