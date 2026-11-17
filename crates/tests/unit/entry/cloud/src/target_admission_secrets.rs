//! A target-admission refusal names the variable a disabled entry came from,
//! never its resolved value, in the error chain and in the doctor's
//! composition check; a literal unknown id is still named (K-SEC5-3).

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_cli::kernel::{
    CommandRegistry, MemberHost, TargetAdmissionError, describe_command_line, kernel_commands,
    read_disabled,
};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_cloud_entry::cli::doctor::composition_failure;
use systemprompt_extension::cli::{CliConfig, CommandDescriptor, CommandError, CommandHost};

use crate::target_host::reference_admit;

const SECRET: &str = "sk-live-kfix8-0f9e8d7c6b5a4f3e";

struct Target {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn target(extensions: &str) -> Target {
    let dir = tempfile::tempdir().expect("tempdir");
    let profile_dir = dir.path().join("profiles").join("prod");
    std::fs::create_dir_all(&profile_dir).expect("profile dir");
    let path = profile_dir.join("profile.yaml");
    std::fs::write(
        &path,
        format!("name: prod\ntarget: cloud\nextensions:\n{extensions}"),
    )
    .expect("write profile");
    Target { _dir: dir, path }
}

fn descriptor() -> CommandDescriptor {
    let declared = systemprompt::reference()
        .declare()
        .expect("the reference composition declares");
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands");
    describe_command_line(&registry, &["cloud".to_owned(), "doctor".to_owned()])
        .expect("cloud doctor is described")
}

fn admit(path: &PathBuf) -> CommandError {
    let ctx = CommandContext::new(CliConfig::default(), EnvOverrides::default())
        .with_admit(Some(reference_admit()));
    MemberHost::new(&ctx, descriptor())
        .admitted_views(path)
        .expect_err("the target does not admit")
}

fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

fn set(name: &str, value: &str) {
    unsafe {
        std::env::set_var(name, value);
    }
}

#[test]
fn an_unknown_id_from_a_variable_is_named_by_the_variable() {
    let var = "SP_KFIX8_DISABLED_SECRET";
    set(var, SECRET);
    let profile = target(&format!("  disabled:\n    - \"${{{var}}}\"\n"));
    let error = admit(&profile.path);
    let text = chain(&error);
    assert!(!text.contains(SECRET), "the value is printed: {text}");
    assert!(text.contains(&format!("${{{var}}}")), "{text}");
    let doctor = composition_failure(&error);
    assert!(!doctor.detail.contains(SECRET), "{}", doctor.detail);
    assert!(doctor.detail.contains(var), "{}", doctor.detail);
}

#[test]
fn a_disabled_field_that_is_not_a_list_is_named_by_the_variable() {
    let var = "SP_KFIX8_DISABLED_SCALAR";
    set(var, SECRET);
    let profile = target(&format!("  disabled: \"${{{var}}}\"\n"));
    let error = read_disabled(&profile.path).expect_err("a string is not a list");
    assert!(
        matches!(&error, TargetAdmissionError::ParseWithheld { variables, .. } if variables == &[var.to_owned()]),
        "{error:?}"
    );
    assert!(!chain(&error).contains(SECRET), "{}", chain(&error));
    let admitted = admit(&profile.path);
    assert!(!chain(&admitted).contains(SECRET));
    assert!(!composition_failure(&admitted).detail.contains(SECRET));
}

#[test]
fn a_literal_unknown_id_is_named_as_written() {
    let profile = target("  disabled:\n    - \"no-such-member-kfix8\"\n");
    let error = admit(&profile.path);
    let text = chain(&error);
    assert!(text.contains("no-such-member-kfix8"), "{text}");
    assert!(composition_failure(&error).detail.contains("no-such-member-kfix8"));
    let literal = target("  disabled: \"no-such-member-kfix8\"\n");
    assert!(matches!(
        read_disabled(&literal.path),
        Err(TargetAdmissionError::Parse { .. })
    ));
}
