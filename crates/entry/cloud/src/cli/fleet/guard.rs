//! The read-only gate for `cloud fleet run`.
//!
//! A forwarded command line under `cloud` is refused because cloud commands
//! run on this machine; any other command is refused unless its descriptor
//! is read-only, preserves data and is not a cloud-operator command (such as
//! `admin session`, which manages this machine's session and runs locally).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Result, bail};

use systemprompt_extension::cli::{DescribedCommand, RoutingClass};

pub fn ensure_read_only(described: &DescribedCommand, command: &[String]) -> Result<()> {
    let DescribedCommand {
        path, descriptor, ..
    } = described;
    if path.first().map(String::as_str) == Some("cloud") {
        bail!(
            "cloud commands run on this machine, not on tenants; fleet run forwards tenant \
             commands"
        );
    }
    if descriptor.cloud_operator()
        || descriptor.data_impact().destroys_data()
        || descriptor.routing_class() != RoutingClass::ReadOnly
    {
        bail!(
            "'{}' is not a read-only command; cloud fleet run forwards read-only commands only",
            command.join(" ")
        );
    }
    Ok(())
}
