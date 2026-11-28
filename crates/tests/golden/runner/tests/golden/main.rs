//! Every golden scenario through the harness's headless run, once per
//! protocol it lists. A run passes when every difference from `expected/`
//! is classified in the reviewed file beside the comparator; what each run
//! left, its trace and its ledger row are written under
//! `<CARGO_TARGET_TMPDIR>/golden/runs`, where `parity-ledger` gathers them.

use std::path::{Path, PathBuf};

use systemprompt_golden_runner::outcome::{failed, record, unclassified};
use systemprompt_golden_runner::{Scenario, drive, judge};
use systemprompt_parity_compare::Classifications;

fn base() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("golden")
}

async fn scenario(id: &str) {
    let scenario = Scenario::by_id(id).unwrap();
    assert!(
        scenario.products.iter().any(|product| product == "harness"),
        "{id} is not a harness scenario"
    );
    let reviewed = Classifications::reviewed().unwrap();
    let mut open = Vec::new();
    for protocol in &scenario.protocols {
        let ran = drive(&scenario, protocol, &base().join("work")).await;
        let outcome = ran
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error:#}"))
            .and_then(|output| judge(&scenario, protocol, output, &reviewed))
            .unwrap_or_else(|error| failed(&scenario, protocol, &error));
        record(
            &base().join("runs"),
            &scenario,
            protocol,
            ran.as_ref().ok(),
            &outcome,
        )
        .unwrap();
        if let Some(error) = &outcome.row.error {
            open.push(format!("{protocol}: the run failed: {error}"));
        }
        for difference in unclassified(&outcome.row) {
            open.push(format!(
                "{protocol}: {} expected {} got {}",
                difference.path, difference.expected, difference.actual
            ));
        }
    }
    assert!(
        open.is_empty(),
        "{id}: unclassified differences:\n{}",
        open.join("\n")
    );
}

macro_rules! golden {
    ($($name:ident => $id:literal),* $(,)?) => {
        $(
            #[tokio::test(flavor = "multi_thread")]
            async fn $name() {
                scenario($id).await;
            }
        )*

        #[test]
        fn every_scenario_of_the_corpus_has_a_test() {
            let listed = [$($id),*];
            let mut found: Vec<String> = std::fs::read_dir(Scenario::corpus())
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.path().join("scenario.toml").is_file())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            found.sort();
            assert_eq!(found, listed);
        }
    };
}

golden! {
    s01_answer_without_tools => "01-answer-without-tools",
    s02_read_file => "02-read-file",
    s03_edit_file => "03-edit-file",
    s04_edit_non_unique_match => "04-edit-non-unique-match",
    s05_write_new_file => "05-write-new-file",
    s06_write_outside_workspace => "06-write-outside-workspace",
    s07_multi_file_patch => "07-multi-file-patch",
    s08_glob_and_grep => "08-glob-and-grep",
    s09_shell_success => "09-shell-success",
    s10_shell_nonzero_exit => "10-shell-nonzero-exit",
    s11_shell_long_output => "11-shell-long-output",
    s12_background_job_abort => "12-background-job-abort",
    s13_denied_by_rule => "13-denied-by-rule",
    s14_approval_approved => "14-approval-approved",
    s15_approval_rejected_feedback => "15-approval-rejected-feedback",
    s16_verdict_deny => "16-verdict-deny",
    s17_verdict_timeout => "17-verdict-timeout",
    s18_todo_list => "18-todo-list",
    s19_subagent_task => "19-subagent-task",
    s20_steering_mid_run => "20-steering-mid-run",
    s21_abort_mid_stream => "21-abort-mid-stream",
    s22_provider_error_then_success => "22-provider-error-then-success",
    s23_compaction => "23-compaction",
    s24_revert_to_message => "24-revert-to-message",
}
