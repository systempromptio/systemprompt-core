//! Probe CLI groups named after the reserved CLI paths.

use systemprompt_extension::cli::{
    CliCommandGroup, CommandDescriptor, CommandError, CommandHost, CommandOutput, DbUrlRouting,
    DescribeCommand,
};

macro_rules! probe_group {
    ($ty:ident, $name:literal) => {
        #[derive(Debug, Clone, clap::Subcommand)]
        pub(crate) enum $ty {
            Status,
        }

        impl DescribeCommand for $ty {
            fn descriptor(&self) -> CommandDescriptor {
                CommandDescriptor::NONE
            }
        }

        impl CliCommandGroup for $ty {
            const NAME: &'static str = $name;

            async fn run(self, _host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
                Err(CommandError::new(std::fmt::Error))
            }
        }
    };
}

probe_group!(AnalyticsGroup, "analytics");
probe_group!(CloudGroup, "cloud");
probe_group!(McpGroup, "mcp");
probe_group!(ProbeGroup, "probe-tools");

#[derive(Debug, Clone, clap::Subcommand)]
pub(crate) enum DirectProbeGroup {
    Status,
}

impl DescribeCommand for DirectProbeGroup {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::NONE
    }
}

impl CliCommandGroup for DirectProbeGroup {
    const NAME: &'static str = "probe-direct";

    fn db_url_routing(&self) -> DbUrlRouting {
        DbUrlRouting::Direct
    }

    async fn run(self, _host: &dyn CommandHost) -> Result<CommandOutput, CommandError> {
        Err(CommandError::new(std::fmt::Error))
    }
}
