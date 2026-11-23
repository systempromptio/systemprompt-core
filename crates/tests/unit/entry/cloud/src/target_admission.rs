//! The target-admission grant: the cloud member reads the open admitted
//! views of a profile at a path without loading that profile, a command
//! without the grant is refused, a disabled member is absent from the views,
//! and an unknown or unresolvable disabled id is a typed error.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use systemprompt_cli::kernel::{
    CommandRegistry, MemberHost, TargetAdmissionError, describe_command_line, kernel_commands,
    read_disabled,
};
use systemprompt_cli::{CommandContext, EnvOverrides};
use systemprompt_extension::cli::{
    AdmittedViews, CliConfig, CommandDescriptor, CommandError, CommandHost, KernelGrant,
};

use crate::target_host::reference_admit;

const TARGET_ONLY: &str = "SP_PX7B_TARGET_ONLY_URL_5d1a";
const DISABLED_VAR: &str = "SP_PX7B_TARGET_ONLY_DISABLED_5d1a";

struct TargetProfile {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

fn target_profile(disabled: &[&str]) -> TargetProfile {
    let dir = tempfile::tempdir().expect("tempdir");
    let profile_dir = dir.path().join("profiles").join("prod");
    std::fs::create_dir_all(&profile_dir).expect("profile dir");
    let path = profile_dir.join("profile.yaml");
    let listed = disabled
        .iter()
        .map(|id| format!("    - \"{id}\"\n"))
        .collect::<String>();
    let disabled_block = if disabled.is_empty() {
        "  disabled: []\n".to_owned()
    } else {
        format!("  disabled:\n{listed}")
    };
    std::fs::write(
        &path,
        format!(
            "name: prod\ntarget: cloud\nserver:\n  external_url: \"${{{TARGET_ONLY}}}\"\n  \
             host: \"${{{TARGET_ONLY}}}\"\ndatabase:\n  url: \"${{{TARGET_ONLY}}}\"\n\
             extensions:\n{disabled_block}"
        ),
    )
    .expect("write profile");
    TargetProfile { _dir: dir, path }
}

fn ctx() -> CommandContext {
    CommandContext::new(CliConfig::default(), EnvOverrides::default())
        .with_admit(Some(reference_admit()))
}

fn cloud_doctor_descriptor() -> CommandDescriptor {
    let declared = systemprompt::reference()
        .declare()
        .expect("the reference composition declares");
    let registry = CommandRegistry::build(&declared.commands(), &kernel_commands())
        .expect("the registry accepts the reference commands");
    describe_command_line(&registry, &["cloud".to_owned(), "doctor".to_owned()])
        .expect("cloud doctor is described")
}

fn member_ids(views: &AdmittedViews) -> Vec<&'static str> {
    views
        .migrations
        .members()
        .iter()
        .map(|(metadata, _)| metadata.id)
        .collect()
}

fn chain(error: &CommandError) -> String {
    let mut text = error.to_string();
    let mut cause = std::error::Error::source(error);
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

#[test]
fn the_cloud_member_reads_the_views_of_a_profile_whose_other_fields_are_target_only() {
    assert!(std::env::var_os(TARGET_ONLY).is_none());
    let target = target_profile(&[]);
    let descriptor = cloud_doctor_descriptor();
    assert!(descriptor.holds(KernelGrant::TargetAdmission));

    let ctx = ctx();
    let views = MemberHost::new(&ctx, descriptor)
        .admitted_views(&target.path)
        .unwrap_or_else(|e| panic!("the grant serves the views: {}", chain(&e)));

    assert!(member_ids(&views).contains(&"notifications"));
    assert!(!views.configs.decls().is_empty() || !views.migrations.members().is_empty());
}

#[test]
fn a_command_without_the_grant_is_refused() {
    let target = target_profile(&[]);
    let ctx = ctx();

    let refused = MemberHost::new(&ctx, CommandDescriptor::NONE)
        .admitted_views(&target.path)
        .expect_err("no grant, no views");
    assert!(
        chain(&refused).contains("`target-admission` kernel grant"),
        "{}",
        chain(&refused)
    );

    let kernel = CommandHost::admitted_views(&ctx, &target.path)
        .expect_err("the kernel host serves no target admission");
    assert!(chain(&kernel).contains("`target-admission` kernel grant"));

    MemberHost::new(
        &ctx,
        CommandDescriptor::NONE.with_grant(KernelGrant::TargetAdmission),
    )
    .admitted_views(&target.path)
    .expect("control: the same context serves a granted command");
}

#[test]
fn a_disabled_member_is_absent_from_the_views() {
    let disabled = target_profile(&["brain", "brain-http", "notifications", "notifications-http"]);
    let ctx = ctx();
    let views = MemberHost::new(&ctx, cloud_doctor_descriptor())
        .admitted_views(&disabled.path)
        .unwrap_or_else(|e| panic!("{}", chain(&e)));
    assert!(!member_ids(&views).contains(&"notifications"));

    let enabled = target_profile(&[]);
    let ctx = self::ctx();
    let views = MemberHost::new(&ctx, cloud_doctor_descriptor())
        .admitted_views(&enabled.path)
        .unwrap_or_else(|e| panic!("{}", chain(&e)));
    assert!(
        member_ids(&views).contains(&"notifications"),
        "control: enabled is present"
    );
}

#[test]
fn an_unknown_disabled_id_is_the_composition_error() {
    let target = target_profile(&["no-such-member-5d1a"]);
    let ctx = ctx();
    let error = MemberHost::new(&ctx, cloud_doctor_descriptor())
        .admitted_views(&target.path)
        .expect_err("an unknown disabled id fails admission");
    let composition = std::error::Error::source(&error)
        .and_then(|source| source.downcast_ref::<systemprompt::CompositionError>())
        .unwrap_or_else(|| panic!("the cause is the composition error: {}", chain(&error)));
    assert!(matches!(
        composition,
        systemprompt::CompositionError::UnknownExtension(entry) if entry.id() == "no-such-member-5d1a"
    ));
}

#[test]
fn an_unresolvable_disabled_id_is_a_typed_error_and_admits_nothing() {
    assert!(std::env::var_os(DISABLED_VAR).is_none());
    let target = target_profile(&[&format!("${{{DISABLED_VAR}}}")]);

    let error = read_disabled(&target.path).expect_err("the disabled list needs the variable");
    match error {
        TargetAdmissionError::Interpolation { source, .. } => {
            assert_eq!(source.variable, DISABLED_VAR);
        },
        other => panic!("expected an interpolation error, got {other:?}"),
    }

    let ctx = ctx();
    MemberHost::new(&ctx, cloud_doctor_descriptor())
        .admitted_views(&target.path)
        .expect_err("the host refuses the profile");
    MemberHost::new(&ctx, cloud_doctor_descriptor())
        .admitted_views(&target_profile(&[]).path)
        .expect("a failed read leaves the admission unspent");
}

#[test]
fn the_admission_is_spent_after_one_use() {
    let target = target_profile(&[]);
    let ctx = ctx();
    let host = MemberHost::new(&ctx, cloud_doctor_descriptor());
    host.admitted_views(&target.path).expect("first admission");
    let spent = host
        .admitted_views(&target.path)
        .expect_err("second admission");
    assert!(chain(&spent).contains("not available"), "{}", chain(&spent));
}
