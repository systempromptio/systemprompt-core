//! A whole top-level command group contributed from outside `systemprompt-cli`
//! through the public kernel surface only: nested subcommands, its own
//! descriptor (cloud-operator, local export, read-only routing), its `--help`
//! slot, and dispatch with the kernel context. The group below names nothing
//! from `systemprompt_cli` but `systemprompt_cli::kernel` and the
//! `register_cli_command!` macro.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::{Args, Subcommand};
use systemprompt_cli::kernel::{
    CliCommandGroup, CommandContext, CommandDescriptor, CommandRegistry, DbUrlRouting,
    DescribeCommand, RoutingClass, describe_command_line,
};

#[derive(Debug, Subcommand)]
enum FleetProbeCommands {
    #[command(subcommand, about = "Nested tenant probes")]
    Tenant(TenantProbeCommands),
    #[command(about = "Read-only report with a local export")]
    Report(ReportArgs),
}

#[derive(Debug, Subcommand)]
enum TenantProbeCommands {
    #[command(about = "Show one tenant")]
    Show(ShowArgs),
}

#[derive(Debug, Args)]
struct ShowArgs {
    #[arg(long)]
    id: String,
}

#[derive(Debug, Args)]
struct ReportArgs {
    #[arg(long)]
    export: Option<String>,
}

impl DescribeCommand for FleetProbeCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Tenant(_) => CommandDescriptor::PROFILE_ONLY.with_cloud_operator(),
            Self::Report(_) => CommandDescriptor::FULL
                .with_read_only()
                .with_remote_eligible()
                .with_local_export(),
        }
    }
}

impl CliCommandGroup for FleetProbeCommands {
    const PARENT: &'static [&'static str] = &[];
    const NAME: &'static str = "fprobe";
    const ABOUT: &'static str = "Registered top-level probe group";
    const DISPLAY_ORDER: usize = 45;

    fn db_url_routing(&self) -> DbUrlRouting {
        match self {
            Self::Tenant(_) => DbUrlRouting::Unsupported,
            Self::Report(_) => DbUrlRouting::Direct,
        }
    }

    fn run(self, _ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send {
        let ran = match self {
            Self::Tenant(TenantProbeCommands::Show(args)) => format!("tenant show {}", args.id),
            Self::Report(args) => format!("report {:?}", args.export),
        };
        std::future::ready(Err(anyhow::anyhow!("fprobe ran: {ran}")))
    }
}

systemprompt_cli::register_cli_command!(FleetProbeCommands);

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

#[test]
fn the_group_is_collected_from_its_registration() {
    let registry = CommandRegistry::collected().expect("collected registrations are disjoint");
    assert!(
        registry
            .registrations()
            .iter()
            .any(|entry| entry.path() == "fprobe" && entry.group().ends_with("FleetProbeCommands")),
        "{registry:?}"
    );
}

#[test]
fn a_nested_subcommand_carries_the_groups_descriptor() {
    let tenant = describe_command_line(&argv(&["fprobe", "tenant", "show", "--id", "t1"]))
        .expect("fprobe tenant show parses");
    assert!(tenant.cloud_operator());
    assert!(tenant.profile() && !tenant.secrets() && !tenant.database());
    assert!(!tenant.local_export());

    let report = describe_command_line(&argv(&["fprobe", "report"])).expect("parses");
    assert!(report.local_export());
    assert!(!report.cloud_operator());
    assert_eq!(report.routing_class(), RoutingClass::ReadOnly);
}

#[test]
fn a_nested_group_without_its_leaf_is_refused() {
    assert!(describe_command_line(&argv(&["fprobe", "tenant"])).is_err());
}

#[tokio::test]
async fn dispatch_runs_the_nested_command_with_the_kernel_context() {
    let registry = CommandRegistry::collected().expect("collected");
    let root = systemprompt_cli::args::command(&registry).expect("assemble");
    let matches = root
        .try_get_matches_from(["systemprompt", "fprobe", "tenant", "show", "--id", "t1"])
        .expect("parses");
    let registered = registry
        .resolve(&matches)
        .expect("resolve")
        .expect("fprobe resolves to its registration");
    assert_eq!(registered.db_url_routing(), DbUrlRouting::Unsupported);
    let ctx = CommandContext::new(
        systemprompt_cli::CliConfig::new().with_interactive(false),
        systemprompt_cli::EnvOverrides::default(),
    );
    let err = registered
        .run(&ctx)
        .await
        .expect_err("probe reports it ran");
    assert_eq!(err.to_string(), "fprobe ran: tenant show t1");
}

#[test]
fn help_lists_the_group_in_its_declared_slot() {
    let registry = CommandRegistry::collected().expect("collected");
    let help = systemprompt_cli::args::command(&registry)
        .expect("assemble")
        .render_help()
        .to_string();
    let position = |name: &str| {
        help.find(&format!("\n  {name} "))
            .unwrap_or_else(|| panic!("`{name}` is listed in --help:\n{help}"))
    };
    assert!(position("admin") < position("fprobe"));
    assert!(position("fprobe") < position("analytics"));
    assert!(position("analytics") < position("web"));
    assert!(position("plugins") < position("build"));
}
