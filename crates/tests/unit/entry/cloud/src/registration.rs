//! The `cloud` group reaches the CLI only through the cloud member's
//! declaration: these assemble the command tree from the reference
//! composition's declared commands exactly as the runner does, so each
//! assertion fails if the declaration, its owner or grant, its `--help` slot,
//! a subcommand path, a flag, a cloud-operator descriptor or a hidden env value
//! is lost.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use clap::{Arg, Command, FromArgMatches};
use systemprompt_cli::args::{self, Cli};
use systemprompt_cli::kernel::{CommandRegistry, describe_command_line, kernel_commands};
use systemprompt_cli::runner::profile_routing::is_cloud_bypass_command;
use systemprompt_cloud_entry::cli::CloudCommands;
use systemprompt_extension::cli::{
    CommandDescriptor, DataImpact, DbUrlRouting, KernelGrant, RoutingClass,
};

const SENTINEL: &str = "sentinel-do-not-leak";

fn argv(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

fn registry() -> CommandRegistry {
    let declared = systemprompt::reference()
        .declare()
        .expect("the reference composition declares");
    CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands")
}

fn root() -> (CommandRegistry, Command) {
    let registry = registry();
    let root = args::command(&registry).expect("assemble the command tree");
    (registry, root)
}

fn descriptor(args: &[&str]) -> CommandDescriptor {
    describe_command_line(&registry(), &argv(args)).unwrap_or_else(|e| panic!("{args:?}: {e:#}"))
}

#[test]
fn the_cloud_member_declares_the_cloud_group_with_the_operator_grant() {
    let declared = systemprompt::reference()
        .declare()
        .expect("the reference composition declares");
    let commands = declared.commands();
    let cloud: Vec<_> = commands
        .commands()
        .iter()
        .filter(|command| command.owner.as_str() == "cloud")
        .map(|command| (command.decl.path(), command.decl.grants().to_vec()))
        .collect();
    assert_eq!(cloud, [(vec!["cloud"], vec![KernelGrant::CloudOperator])]);
}

#[test]
fn help_lists_cloud_between_admin_and_web() {
    let (_, root) = root();
    let help = root.clone().render_help().to_string();
    let position = |name: &str| {
        help.find(&format!("\n  {name} "))
            .unwrap_or_else(|| panic!("`{name}` is listed in --help:\n{help}"))
    };
    assert!(position("infra") < position("admin"));
    assert!(position("admin") < position("cloud"));
    assert!(position("cloud") < position("web"));
    let cloud = root.find_subcommand("cloud").expect("cloud");
    assert_eq!(
        cloud.get_about().map(ToString::to_string).as_deref(),
        Some("Cloud deployment, backup, and setup")
    );
}

#[test]
fn every_cloud_subcommand_path_is_reachable() {
    let (_, root) = root();
    let cloud = root.find_subcommand("cloud").expect("cloud");
    let names: Vec<&str> = cloud.get_subcommands().map(Command::get_name).collect();
    assert_eq!(
        names,
        [
            "auth",
            "init",
            "tenant",
            "profile",
            "deploy",
            "backup",
            "doctor",
            "status",
            "fleet",
            "dockerfile",
            "sync"
        ]
    );
    for path in [
        &["auth", "login"][..],
        &["auth", "logout"],
        &["auth", "whoami"],
        &["tenant", "list"],
        &["profile", "list"],
        &["fleet", "run"],
        &["fleet", "versions"],
    ] {
        let mut cmd = cloud;
        for name in path {
            cmd = cmd
                .find_subcommand(name)
                .unwrap_or_else(|| panic!("cloud {path:?} is reachable"));
        }
    }
}

#[test]
fn every_cloud_descriptor_is_a_cloud_operator_and_bypasses_routing() {
    for args in [
        &["cloud", "status"][..],
        &["cloud", "doctor"],
        &["cloud", "deploy"],
        &["cloud", "backup"],
        &["cloud", "init"],
        &["cloud", "dockerfile"],
        &["cloud", "auth", "whoami"],
        &["cloud", "tenant", "list"],
        &["cloud", "profile", "list"],
    ] {
        let descriptor = descriptor(args);
        assert!(descriptor.cloud_operator(), "{args:?}");
        assert!(is_cloud_bypass_command(&descriptor), "{args:?}");
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
    }
}

#[test]
fn bootstrap_requirements_match_each_subcommand() {
    let deploy = descriptor(&["cloud", "deploy"]);
    assert!(!deploy.profile() && !deploy.secrets() && !deploy.database());

    for args in [&["cloud", "status"][..], &["cloud", "backup"]] {
        let d = descriptor(args);
        assert!(d.profile() && !d.secrets() && !d.database(), "{args:?}");
    }

    let sync = descriptor(&["cloud", "sync", "export"]);
    assert!(sync.profile() && sync.database());
    assert_eq!(sync.routing_class(), RoutingClass::ReadOnly);

    let init = descriptor(&["cloud", "init"]);
    assert!(!init.profile() && !init.database());
}

#[test]
fn cloud_refuses_a_direct_database_url() {
    let (registry, root) = root();
    let matches = root
        .try_get_matches_from(["systemprompt", "cloud", "status"])
        .expect("cloud status parses");
    let registered = registry
        .resolve(&matches)
        .expect("resolve")
        .expect("cloud resolves to its registration");
    assert_eq!(
        registered.parsed.db_url_routing(),
        DbUrlRouting::Unsupported
    );
}

fn cloud_profile(args: &[&str]) -> Result<(Cli, Option<String>), clap::Error> {
    let (_, root) = root();
    let matches =
        root.try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))?;
    let cli = Cli::from_globals(&matches)?;
    let cloud = matches
        .subcommand_matches("cloud")
        .expect("a cloud command");
    let profile = match CloudCommands::from_arg_matches(cloud)? {
        CloudCommands::Doctor { profile, .. }
        | CloudCommands::Deploy { profile, .. }
        | CloudCommands::Backup { profile, .. } => profile,
        other => panic!("expected a cloud doctor, deploy or backup command, got {other:?}"),
    };
    Ok((cli, profile))
}

#[test]
fn cloud_profile_flags_parse_alongside_the_global_flag() {
    for args in [
        ["cloud", "doctor", "--profile", "prod"],
        ["cloud", "deploy", "--profile", "prod"],
        ["cloud", "backup", "--profile", "prod"],
        ["cloud", "doctor", "-p", "prod"],
    ] {
        let (cli, profile) = cloud_profile(&args).unwrap_or_else(|e| panic!("parse {args:?}: {e}"));
        assert_eq!(profile.as_deref(), Some("prod"), "{args:?}");
        assert_eq!(
            cli.profile_opts.profile.as_deref(),
            Some("prod"),
            "{args:?} must also select the profile the CLI bootstraps"
        );
    }
}

#[test]
fn cloud_profile_flags_refuse_a_path() {
    for args in [
        ["cloud", "doctor", "--profile", "/tmp/profiles/prod"],
        ["cloud", "deploy", "--profile", "profiles/prod"],
        ["cloud", "backup", "--profile", "prod name"],
    ] {
        let err = cloud_profile(&args).expect_err("a profile path is a usage error");
        assert_eq!(
            err.kind(),
            clap::error::ErrorKind::ValueValidation,
            "{args:?}"
        );
    }
}

#[test]
fn the_global_profile_flag_accepts_a_path_before_cloud() {
    let (_, root) = root();
    let matches = root
        .try_get_matches_from([
            "systemprompt",
            "--profile",
            "/tmp/profiles/prod",
            "cloud",
            "status",
        ])
        .expect("the global flag resolves names and paths");
    let cli = Cli::from_globals(&matches).expect("globals");
    assert_eq!(
        cli.profile_opts.profile.as_deref(),
        Some("/tmp/profiles/prod")
    );
}

fn leaking_env_args(cmd: &Command, path: &str, leaking: &mut Vec<String>) {
    for arg in cmd.get_arguments() {
        if arg.get_env().is_none() {
            continue;
        }
        if !arg.is_hide_env_values_set() && !arg.is_hide_env_set() && !arg.is_hide_set() {
            leaking.push(format!("{path} {}", arg.get_id()));
        }
    }
    for sub in cmd.get_subcommands() {
        leaking_env_args(sub, &format!("{path} {}", sub.get_name()), leaking);
    }
}

#[test]
fn no_env_bound_cloud_arg_renders_its_value_in_help() {
    let (_, root) = root();
    let cloud = root.find_subcommand("cloud").expect("cloud");
    let mut bound = Vec::new();
    let mut leaking = Vec::new();
    collect_env_bound(cloud, &mut bound);
    leaking_env_args(cloud, "systemprompt cloud", &mut leaking);
    assert!(
        bound.iter().any(|env| env == "ANTHROPIC_API_KEY"),
        "the walk reaches `cloud profile create`: {bound:?}"
    );
    assert!(
        leaking.is_empty(),
        "these env-bound args would print their live value in --help; \
         add `hide_env_values = true`: {leaking:#?}"
    );
}

fn collect_env_bound(cmd: &Command, out: &mut Vec<String>) {
    out.extend(
        cmd.get_arguments()
            .filter_map(Arg::get_env)
            .map(|env| env.to_string_lossy().into_owned()),
    );
    for sub in cmd.get_subcommands() {
        collect_env_bound(sub, out);
    }
}

#[test]
fn cloud_profile_create_help_names_the_provider_vars_without_their_values() {
    unsafe {
        std::env::set_var("ANTHROPIC_API_KEY", SENTINEL);
    }
    let (_, mut root) = root();
    let create = root
        .find_subcommand_mut("cloud")
        .expect("cloud")
        .find_subcommand_mut("profile")
        .expect("profile")
        .find_subcommand_mut("create")
        .expect("create");
    let help = create.render_long_help().to_string();
    assert!(
        !help.contains(SENTINEL),
        "cloud profile create --help leaked an env value:\n{help}"
    );
    assert!(help.contains("ANTHROPIC_API_KEY"), "{help}");
}
