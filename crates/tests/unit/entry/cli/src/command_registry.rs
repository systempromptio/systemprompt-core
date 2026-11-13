#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::{CommandFactory, Subcommand};
use systemprompt_cli::args::{self, Cli};
use systemprompt_cli::kernel::{
    CliCommandGroup, CliCommandLeaf, CliCommandRegistration, CommandContext, CommandDescriptor,
    CommandRegistry, DataImpact, DbUrlRouting, DescribeCommand, RegistryError,
};
use systemprompt_cli::{CliConfig, EnvOverrides};

const PROBE_RAN: &str = "probe group ran";

#[derive(Debug, Subcommand)]
enum ProbeCommands {
    #[command(about = "Probe without bootstrap")]
    Ping,
    #[command(about = "Probe that needs the database")]
    Query,
}

impl DescribeCommand for ProbeCommands {
    fn descriptor(&self) -> CommandDescriptor {
        match self {
            Self::Ping => CommandDescriptor::NONE,
            Self::Query => CommandDescriptor::FULL.with_data_impact(DataImpact::Destructive),
        }
    }
}

impl CliCommandGroup for ProbeCommands {
    const PARENT: &'static [&'static str] = &[];
    const NAME: &'static str = "probe";
    const ABOUT: &'static str = "Registered probe group";

    fn db_url_routing(&self) -> DbUrlRouting {
        match self {
            Self::Ping => DbUrlRouting::Unsupported,
            Self::Query => DbUrlRouting::Direct,
        }
    }

    fn run(self, _ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send {
        std::future::ready(Err(anyhow::anyhow!("{PROBE_RAN}: {self:?}")))
    }
}

#[derive(Debug, Subcommand)]
enum OtherProbeCommands {
    #[command(about = "Second probe")]
    Ping,
}

impl DescribeCommand for OtherProbeCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::PROFILE_ONLY
    }
}

impl CliCommandGroup for OtherProbeCommands {
    const PARENT: &'static [&'static str] = &[];
    const NAME: &'static str = "probe";
    const ABOUT: &'static str = "Second probe group on the same path";

    fn run(self, _ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send {
        std::future::ready(Ok(()))
    }
}

#[derive(Debug, Subcommand)]
enum AdminProbeCommands {
    #[command(about = "Nested probe")]
    Ping,
}

impl DescribeCommand for AdminProbeCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::PROFILE_AND_SECRETS
    }
}

impl CliCommandGroup for AdminProbeCommands {
    const PARENT: &'static [&'static str] = &["admin"];
    const NAME: &'static str = "aprobe";
    const ABOUT: &'static str = "Probe nested under admin";

    fn run(self, _ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send {
        std::future::ready(Ok(()))
    }
}

#[derive(Debug, Subcommand)]
enum ShadowingCommands {
    #[command(about = "Shadows admin")]
    Ping,
}

impl DescribeCommand for ShadowingCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::NONE
    }
}

impl CliCommandGroup for ShadowingCommands {
    const PARENT: &'static [&'static str] = &[];
    const NAME: &'static str = "admin";
    const ABOUT: &'static str = "Shadows the kernel admin group";

    fn run(self, _ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send {
        std::future::ready(Ok(()))
    }
}

#[derive(Debug, Subcommand)]
enum OrphanCommands {
    #[command(about = "Orphan")]
    Ping,
}

impl DescribeCommand for OrphanCommands {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::NONE
    }
}

impl CliCommandGroup for OrphanCommands {
    const PARENT: &'static [&'static str] = &["no-such-group"];
    const NAME: &'static str = "orphan";
    const ABOUT: &'static str = "Names a parent that does not exist";

    fn run(self, _ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send {
        std::future::ready(Ok(()))
    }
}

fn registry(entries: Vec<CliCommandRegistration>) -> CommandRegistry {
    CommandRegistry::from_registrations(entries).expect("registrations are disjoint")
}

fn matches(registry: &CommandRegistry, argv: &[&str]) -> clap::ArgMatches {
    registry
        .assemble(Cli::command())
        .expect("assemble")
        .try_get_matches_from(std::iter::once("systemprompt").chain(argv.iter().copied()))
        .expect("argv parses")
}

fn top_level_names(registry: &CommandRegistry) -> Vec<String> {
    registry
        .assemble(Cli::command())
        .expect("assemble")
        .get_subcommands()
        .map(|sub| sub.get_name().to_owned())
        .collect()
}

#[test]
fn a_registered_group_parses_and_resolves_with_its_declared_descriptor() {
    let registry = registry(vec![CliCommandRegistration::of::<ProbeCommands>()]);

    let ping = registry
        .resolve(&matches(&registry, &["probe", "ping"]))
        .expect("resolve")
        .expect("probe ping is a registered command");
    let desc = ping.descriptor();
    assert!(!desc.profile() && !desc.secrets() && !desc.paths() && !desc.database());
    assert_eq!(ping.db_url_routing(), DbUrlRouting::Unsupported);

    let query = registry
        .resolve(&matches(&registry, &["probe", "query"]))
        .expect("resolve")
        .expect("probe query is a registered command");
    assert!(query.descriptor().database());
    assert_eq!(query.descriptor().data_impact(), DataImpact::Destructive);
    assert_eq!(query.db_url_routing(), DbUrlRouting::Direct);
}

#[test]
fn a_kernel_command_does_not_resolve_to_a_registration() {
    let registry = registry(vec![CliCommandRegistration::of::<ProbeCommands>()]);
    let resolved = registry
        .resolve(&matches(&registry, &["admin", "session", "show"]))
        .expect("resolve");
    assert!(resolved.is_none());
}

#[tokio::test]
async fn dispatch_runs_the_registered_group_with_the_kernel_context() {
    let registry = registry(vec![CliCommandRegistration::of::<ProbeCommands>()]);
    let registered = registry
        .resolve(&matches(&registry, &["probe", "query"]))
        .expect("resolve")
        .expect("registered");
    let ctx = CommandContext::new(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
    );

    let err = registered
        .run(&ctx)
        .await
        .expect_err("probe reports it ran");
    assert_eq!(err.to_string(), format!("{PROBE_RAN}: Query"));
}

#[test]
fn a_group_registers_under_an_existing_kernel_group() {
    let registry = registry(vec![CliCommandRegistration::of::<AdminProbeCommands>()]);
    let resolved = registry
        .resolve(&matches(&registry, &["admin", "aprobe", "ping"]))
        .expect("resolve")
        .expect("admin aprobe is registered");
    assert!(resolved.descriptor().secrets());

    let kernel = registry
        .resolve(&matches(&registry, &["admin", "session", "show"]))
        .expect("resolve");
    assert!(kernel.is_none(), "admin's kernel subcommands still parse");
}

#[test]
fn two_registrations_of_one_path_are_refused_naming_both() {
    let err = CommandRegistry::from_registrations(vec![
        CliCommandRegistration::of::<ProbeCommands>(),
        CliCommandRegistration::of::<OtherProbeCommands>(),
    ])
    .expect_err("duplicate path");

    let RegistryError::Duplicate {
        path,
        first,
        second,
    } = &err
    else {
        panic!("expected a duplicate error, got {err:?}");
    };
    assert_eq!(path, "probe");
    let message = err.to_string();
    assert!(message.contains("ProbeCommands") && message.contains("OtherProbeCommands"));
    assert_ne!(first, second);
}

#[test]
fn a_registration_that_shadows_a_kernel_command_is_refused() {
    let err = registry(vec![CliCommandRegistration::of::<ShadowingCommands>()])
        .assemble(Cli::command())
        .expect_err("admin is a kernel command");
    assert!(matches!(err, RegistryError::ShadowsKernel { ref path, .. } if path == "admin"));
}

#[test]
fn a_registration_under_a_missing_parent_is_refused() {
    let err = registry(vec![CliCommandRegistration::of::<OrphanCommands>()])
        .assemble(Cli::command())
        .expect_err("parent does not exist");
    assert!(
        matches!(err, RegistryError::UnknownParent { ref path, .. } if path == "no-such-group orphan")
    );
}

#[test]
fn help_lists_registered_groups_after_the_kernel_in_a_fixed_order() {
    let forward = registry(vec![
        CliCommandRegistration::of::<ProbeCommands>(),
        CliCommandRegistration::of::<AdminProbeCommands>(),
    ]);
    let reverse = registry(vec![
        CliCommandRegistration::of::<AdminProbeCommands>(),
        CliCommandRegistration::of::<ProbeCommands>(),
    ]);

    let names = top_level_names(&forward);
    assert_eq!(names, top_level_names(&reverse));
    assert_eq!(names.last().map(String::as_str), Some("probe"));
    let kernel: Vec<String> = Cli::command()
        .get_subcommands()
        .map(|sub| sub.get_name().to_owned())
        .collect();
    assert_eq!(&names[..kernel.len()], kernel.as_slice());

    let help = forward
        .assemble(Cli::command())
        .expect("assemble")
        .render_help()
        .to_string();
    assert!(help.contains("probe") && help.contains("Registered probe group"));
}

#[test]
fn the_build_group_is_registered_through_the_seam() {
    let registry = CommandRegistry::collected().expect("collected registrations are disjoint");
    assert!(
        registry
            .registrations()
            .iter()
            .any(|entry| entry.path() == "build" && entry.group().ends_with("BuildCommands")),
        "{registry:?}"
    );
    assert!(
        Cli::command().find_subcommand("build").is_none(),
        "build is not named by the kernel enum"
    );

    let root = args::command(&registry).expect("assemble");
    let build = registry
        .resolve(
            &root
                .try_get_matches_from(["systemprompt", "build", "mcp"])
                .expect("build mcp parses"),
        )
        .expect("resolve")
        .expect("build resolves to its registration");
    let desc = build.descriptor();
    assert!(desc.profile());
    assert!(!desc.secrets() && !desc.paths() && !desc.database());
    assert_eq!(desc.data_impact(), DataImpact::Preserving);
    assert_eq!(build.db_url_routing(), DbUrlRouting::Unsupported);
}

#[derive(Debug, clap::Args)]
struct LeafProbeArgs {
    #[arg(long)]
    port: Option<u16>,
}

impl DescribeCommand for LeafProbeArgs {
    fn descriptor(&self) -> CommandDescriptor {
        CommandDescriptor::PROFILE_SECRETS_AND_PATHS.with_serves_api()
    }
}

impl CliCommandLeaf for LeafProbeArgs {
    const PARENT: &'static [&'static str] = &["infra", "services"];
    const NAME: &'static str = "lprobe";
    const ABOUT: &'static str = "Single registered command";

    fn run(self, _ctx: &CommandContext) -> impl Future<Output = anyhow::Result<()>> + Send {
        std::future::ready(Err(anyhow::anyhow!("leaf ran on {:?}", self.port)))
    }
}

#[tokio::test]
async fn a_single_command_registers_under_a_nested_kernel_group() {
    let registry = registry(vec![CliCommandRegistration::leaf::<LeafProbeArgs>()]);
    let registered = registry
        .resolve(&matches(
            &registry,
            &["infra", "services", "lprobe", "--port", "9"],
        ))
        .expect("resolve")
        .expect("infra services lprobe is registered");
    assert!(registered.descriptor().serves_api());

    let ctx = CommandContext::new(
        CliConfig::new().with_interactive(false),
        EnvOverrides::default(),
    );
    let err = registered.run(&ctx).await.expect_err("leaf reports it ran");
    assert_eq!(err.to_string(), "leaf ran on Some(9)");
}
