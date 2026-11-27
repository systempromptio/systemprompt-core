//! An incomplete purge fails `admin users delete --purge` and carries its
//! output, so a script checking the exit code is not told the purge
//! succeeded; a complete purge succeeds.

use systemprompt_extension::cli::CommandError;
use systemprompt_identifiers::{ExtensionId, UserId};
use systemprompt_identity_entry::cli::users::purge_output;
use systemprompt_users::{PurgeCount, PurgeOutcome, SkippedPurge};

fn counted() -> Vec<PurgeCount> {
    vec![PurgeCount {
        owner: ExtensionId::new("users"),
        table: "users".to_owned(),
        rows: 1,
        absent: false,
    }]
}

fn failed(outcome: &PurgeOutcome) -> CommandError {
    let error = purge_output("purge-probe", &UserId::new("u-purge-probe"), outcome)
        .expect_err("an incomplete purge fails the command");
    error
        .downcast::<CommandError>()
        .expect("the failure is a CommandError")
}

#[test]
fn an_unrecorded_member_fails_the_purge_with_its_output() {
    let error = failed(&PurgeOutcome {
        counts: counted(),
        skipped: vec![],
        unrecorded_members: vec![ExtensionId::new("probe-disabled")],
    });
    assert!(error.output().is_some(), "the purge output is carried");
    assert!(error.to_string().contains("incomplete"), "{error}");
}

#[test]
fn a_skipped_table_fails_the_purge() {
    let error = failed(&PurgeOutcome {
        counts: counted(),
        skipped: vec![SkippedPurge {
            owner: ExtensionId::new("probe-gone"),
            table: "probe_gone".to_owned(),
        }],
        unrecorded_members: vec![],
    });
    assert!(error.output().is_some());
}

#[test]
fn a_complete_purge_succeeds() {
    purge_output(
        "purge-probe",
        &UserId::new("u-purge-probe"),
        &PurgeOutcome {
            counts: counted(),
            skipped: vec![],
            unrecorded_members: vec![],
        },
    )
    .expect("negative control: a complete purge succeeds");
}
