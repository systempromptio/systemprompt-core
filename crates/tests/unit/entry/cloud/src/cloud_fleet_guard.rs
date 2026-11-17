//! `cloud fleet run` forwards read-only commands only: the guard classifies
//! the forwarded command through the host's `describe_command_line` before
//! any tenant is contacted, so a line that selects no command or a command
//! whose owner the profile disables is refused before the read-only check.

use std::path::Path;
use std::sync::Arc;

use systemprompt::traits::DatabaseHandle;
use systemprompt_cli::ScriptedPrompter;
use systemprompt_cli::kernel::{CommandRegistry, classify_command_line, kernel_commands};
use systemprompt_cloud_entry::cli::fleet::ensure_read_only;
use systemprompt_extension::Resolved;
use systemprompt_extension::cli::{
    AdmittedViews, BoxFuture, ChildSupervisor, CliConfig, CommandError, CommandHost, CommandOutput,
    DescribedCommand, Prompter,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_loader::subprocess::ForegroundChildren;
use systemprompt_manifest::{Config, Profile};

struct ClassifyingHost {
    registry: CommandRegistry,
    profile: Profile,
    cli: CliConfig,
    prompter: ScriptedPrompter,
    children: ForegroundChildren,
}

impl ClassifyingHost {
    fn new(disabled: &[&str]) -> Self {
        let declared = systemprompt::reference()
            .declare()
            .expect("the reference composition declares");
        let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
            .expect("the registry accepts the reference commands");
        let mut profile = (*systemprompt_test_fixtures::test_boot_profile()).clone();
        profile.extensions.disabled = disabled.iter().map(|id| ExtensionId::new(*id)).collect();
        Self {
            registry,
            profile,
            cli: CliConfig::default(),
            prompter: ScriptedPrompter::new(Vec::<String>::new()),
            children: ForegroundChildren::default(),
        }
    }
}

fn unavailable() -> CommandError {
    CommandError::new(anyhow::anyhow!("the classifying host runs no command"))
}

impl CommandHost for ClassifyingHost {
    fn output(&self) -> &CliConfig {
        &self.cli
    }

    fn prompter(&self) -> &dyn Prompter {
        &self.prompter
    }

    fn is_database_scoped(&self) -> bool {
        false
    }

    fn profile(&self) -> Result<&Profile, CommandError> {
        Ok(&self.profile)
    }

    fn profile_path(&self) -> Result<&Path, CommandError> {
        Err(unavailable())
    }

    fn db(&self) -> BoxFuture<'_, Result<Arc<dyn DatabaseHandle>, CommandError>> {
        Box::pin(async { Err(unavailable()) })
    }

    fn resolved(&self) -> BoxFuture<'_, Result<Resolved<'_>, CommandError>> {
        Box::pin(async { Err(unavailable()) })
    }

    fn children(&self) -> &dyn ChildSupervisor {
        &self.children
    }

    fn render(&self, _output: &CommandOutput) {}

    fn describe_command_line(&self, command: &[String]) -> Result<DescribedCommand, CommandError> {
        classify_command_line(&self.registry, &self.profile, command).map_err(CommandError::new)
    }

    fn env_profile(&self) -> Option<&str> {
        None
    }

    fn boot_config<'a>(
        &'a self,
        _profile: &'a Path,
    ) -> BoxFuture<'a, Result<Arc<Config>, CommandError>> {
        Box::pin(async { Err(unavailable()) })
    }

    fn admitted_views(&self, _profile: &Path) -> Result<AdmittedViews, CommandError> {
        Err(unavailable())
    }
}

fn args(command: &[&str]) -> Vec<String> {
    command.iter().map(|arg| (*arg).to_owned()).collect()
}

fn guard_with(host: &ClassifyingHost, command: &[String]) -> anyhow::Result<()> {
    let described = host.describe_command_line(command)?;
    ensure_read_only(&described, command)
}

fn guard(command: &[String]) -> anyhow::Result<()> {
    guard_with(&ClassifyingHost::new(&[]), command)
}

#[test]
fn a_disabled_owners_command_is_refused() {
    let overview = args(&["analytics", "overview"]);
    assert!(
        guard_with(&ClassifyingHost::new(&["analytics-http"]), &overview).is_err(),
        "the profile disables the command's owner"
    );
    guard_with(&ClassifyingHost::new(&[]), &overview)
        .expect("negative control: analytics overview is read-only while its owner is enabled");
}

#[test]
fn a_read_only_log_query_is_forwarded() {
    guard(&args(&["infra", "logs", "summary"])).expect("logs summary is read-only");
}

#[test]
fn a_destructive_command_is_refused() {
    let err = guard(&args(&["infra", "db", "migrate"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn a_mutating_command_is_refused() {
    let err = guard(&args(&["infra", "jobs", "list"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn a_local_only_command_is_refused() {
    let err = guard(&args(&["infra", "services", "status"])).unwrap_err();
    assert!(
        err.to_string().contains("not a read-only command"),
        "{err:#}"
    );
}

#[test]
fn cloud_commands_are_refused() {
    let err = guard(&args(&["cloud", "status"])).unwrap_err();
    assert!(err.to_string().contains("run on this machine"), "{err:#}");
}

#[test]
fn an_unknown_command_is_refused() {
    let err = guard(&args(&["no-such-command"])).unwrap_err();
    assert!(
        err.to_string().contains("is not a systemprompt command"),
        "{err:#}"
    );
}

#[test]
fn an_empty_command_is_refused() {
    assert!(guard(&[]).is_err());
}

#[test]
fn irreversible_commands_are_kept_out_of_fleet_run() {
    for command in [
        &["infra", "db", "migrate-down", "users", "1"][..],
        &["infra", "db", "assign-admin", "u"],
        &["admin", "keys", "rotate", "--revoke"],
        &["admin", "users", "role", "promote", "u"],
    ] {
        let err = guard(&args(command)).unwrap_err();
        assert!(
            err.to_string().contains("not a read-only command"),
            "{command:?}: {err:#}"
        );
    }
}

#[test]
fn session_commands_are_refused_as_local_commands_not_as_cloud_commands() {
    for command in [
        &["admin", "session", "list"][..],
        &["admin", "session", "show"],
    ] {
        let err = guard(&args(command)).unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("not a read-only command"),
            "{command:?}: {err:#}"
        );
        assert!(!message.contains("cloud commands"), "{command:?}: {err:#}");
    }
}

#[test]
fn cloud_commands_behind_global_flags_are_refused_as_cloud_commands() {
    let err = guard(&args(&["--json", "cloud", "status"])).unwrap_err();
    assert!(err.to_string().contains("run on this machine"), "{err:#}");
}
