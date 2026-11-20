//! The reference composition admits `files-http` as its conferral describes:
//! no route set (`/api/v1/files` stays reserved to the kernel), the
//! `core files` group with no grants, and a requirement on the
//! `Arc<FileRepository>` the `files` member provides. Deleting a file is
//! destructive and the read commands preserve data. A profile disabling
//! `files-http` refuses `core files` at dispatch; one disabling `files`
//! refuses boot, since `files` is a required member.

use std::sync::Arc;

use systemprompt::{CompositionError, reference};
use systemprompt_cli::CliRunError;
use systemprompt_cli::kernel::{
    CommandRegistry, ResolvedCommand, describe_command_line, kernel_commands,
};
use systemprompt_cli::runner::refuse_disabled;
use systemprompt_extension::Admitted;
use systemprompt_extension::cli::{CommandDescriptor, DataImpact};

use crate::probe::{build, profile};

const OWNER: &str = "files-http";

fn admitted() -> Admitted {
    build(reference()).expect("the reference composition admits")
}

fn registry() -> CommandRegistry {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands")
}

fn describe(args: &[&str]) -> CommandDescriptor {
    let argv: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    describe_command_line(&registry(), &argv).unwrap_or_else(|e| panic!("{args:?}: {e}"))
}

fn resolved(args: &[&str]) -> ResolvedCommand {
    let registry = registry();
    let matches = systemprompt_cli::args::command(&registry)
        .expect("the reference commands attach")
        .try_get_matches_from(std::iter::once("systemprompt").chain(args.iter().copied()))
        .expect("the command parses");
    registry
        .resolve(&matches)
        .expect("the command resolves")
        .expect("the command is a contributed command")
}

#[test]
fn files_http_holds_no_route_set() {
    let admitted = admitted();
    assert!(
        admitted
            .routes()
            .iter()
            .all(|set| set.owner.as_str() != OWNER),
        "files-http contributes no routes"
    );
}

#[test]
fn files_http_requires_the_file_repository() {
    let admitted = admitted();
    let member = admitted
        .built()
        .members()
        .find(|member| member.owner.as_str() == OWNER)
        .expect("files-http is admitted");
    let required: Vec<&str> = member
        .sink
        .requires()
        .iter()
        .map(|requirement| requirement.type_name())
        .collect();
    assert_eq!(
        required,
        [std::any::type_name::<Arc<systemprompt::files::FileRepository>>()]
    );
}

#[test]
fn files_http_declares_core_files_without_grants() {
    let declared = reference()
        .declare()
        .expect("the reference composition declares");
    let commands = declared.commands();
    let owned: Vec<(Vec<&str>, usize)> = commands
        .commands()
        .iter()
        .filter(|command| command.owner.as_str() == OWNER)
        .map(|command| (command.decl.path(), command.decl.grants().len()))
        .collect();
    assert_eq!(owned, [(vec!["core", "files"], 0)]);
}

#[test]
fn deleting_a_file_is_destructive_and_the_reads_preserve_data() {
    let delete = describe(&["core", "files", "delete", "f", "--yes"]);
    assert!(delete.is_destructive());
    assert_eq!(delete.data_impact(), DataImpact::Destructive);
    for args in [
        ["core", "files", "list"].as_slice(),
        &["core", "files", "stats"],
        &["core", "files", "ai", "count"],
        &["core", "files", "config"],
    ] {
        let descriptor = describe(args);
        assert!(!descriptor.is_destructive(), "{args:?}");
        assert_eq!(descriptor.data_impact(), DataImpact::Preserving, "{args:?}");
        assert!(descriptor.database(), "{args:?}");
    }
}

#[test]
fn a_profile_disabling_files_http_refuses_core_files_at_dispatch() {
    let command = resolved(&["core", "files", "list"]);
    let error = refuse_disabled(&command, &profile(&[OWNER]))
        .expect_err("the profile disables the command's owner");
    assert!(matches!(
        error,
        CliRunError::CommandDisabled { path, owner }
            if path == ["core", "files"] && owner.as_str() == OWNER
    ));
    assert!(
        refuse_disabled(&command, &profile(&[])).is_ok(),
        "negative control"
    );
}

#[test]
fn a_profile_disabling_files_refuses_boot() {
    let error = reference()
        .build(&profile(&["files"]))
        .expect_err("files is a required member");
    assert!(matches!(
        &error,
        CompositionError::RequiredDisabled(id) if id.as_str() == "files"
    ));
}
