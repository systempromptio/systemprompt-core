//! Enrolling named hosts through a real `BridgeContext`, with no gateway
//! reachable — so every local host takes the failure arm and the
//! classification arms above it are what is under test.

use systemprompt_bridge::context::{BridgeContext, ProxyMode};
use systemprompt_bridge::integration::enrol::{Outcome, Selection, enrol_hosts};
use systemprompt_bridge::integration::reapply::ModelProtocolOverrides;
use tempfile::TempDir;

fn in_sandbox<R>(f: impl FnOnce() -> R) -> R {
    let home = TempDir::new().expect("home");
    let root = home.path().display().to_string();
    let out = temp_env::with_vars(
        [
            ("HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(format!("{root}/.config"))),
            ("XDG_STATE_HOME", Some(format!("{root}/.state"))),
            ("XDG_DATA_HOME", Some(format!("{root}/.data"))),
            ("XDG_CACHE_HOME", Some(format!("{root}/.cache"))),
            ("SP_BRIDGE_PAT", None),
            ("SP_BRIDGE_CONFIG", None),
            ("SUDO_USER", None),
        ],
        f,
    );
    drop(home);
    out
}

fn context() -> std::sync::Arc<BridgeContext> {
    BridgeContext::start(ProxyMode::Attach).expect("attach context")
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

#[test]
fn enrolling_an_empty_selection_reports_nothing_and_is_not_an_error() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::Ids(Vec::new()),
                &ModelProtocolOverrides::new(),
                None,
            ))
            .expect("an empty selection is valid");
        assert!(reports.is_empty());
    });
}

#[test]
fn an_unknown_host_id_fails_the_whole_request_before_any_host_is_touched() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let err = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::Ids(vec!["not-a-host".to_owned()]),
                &ModelProtocolOverrides::new(),
                None,
            ))
            .expect_err("an unknown id is rejected outright");
        assert!(err.contains("not-a-host"), "got {err}");
        assert!(err.contains("known ids:"), "got {err}");
    });
}

#[test]
fn a_host_the_instance_does_not_enable_is_skipped_rather_than_attempted() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("an empty enabled list is a valid instance state");

        assert!(!reports.is_empty(), "there is at least one local host");
        for report in &reports {
            assert!(
                matches!(report.outcome, Outcome::NotEnabled),
                "with nothing enabled every local host is skipped, got {:?} for {}",
                report.outcome,
                report.host_id
            );
            assert!(!report.is_failure(), "a skipped host is not a failure");
        }
    });
}

#[test]
fn an_enabled_list_naming_a_host_lets_that_one_through_while_skipping_the_rest() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let all = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("baseline");
        let Some(first) = all.first().map(|r| r.host_id.clone()) else {
            return;
        };

        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(vec![first.clone()]),
            ))
            .expect("enrol with one host enabled");

        let named = reports
            .iter()
            .find(|r| r.host_id == first)
            .expect("the named host is reported");
        assert!(
            !matches!(named.outcome, Outcome::NotEnabled),
            "the enabled host must be attempted, not skipped"
        );

        for other in reports.iter().filter(|r| r.host_id != first) {
            assert!(
                matches!(other.outcome, Outcome::NotEnabled | Outcome::SyncOnly),
                "every host not on the enabled list is skipped, got {:?} for {}",
                other.outcome,
                other.host_id
            );
        }
    });
}

#[test]
fn with_no_gateway_reachable_an_enabled_host_reports_a_failure_that_names_a_cause() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let all = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("baseline");
        let Some(first) = all.first().map(|r| r.host_id.clone()) else {
            return;
        };

        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::Ids(vec![first.clone()]),
                &ModelProtocolOverrides::new(),
                None,
            ))
            .expect("the request itself is understood");

        let report = reports.first().expect("one host, one report");
        assert_eq!(report.host_id, first);
        match &report.outcome {
            Outcome::Failed(message) => assert!(
                !message.is_empty(),
                "a failure must carry the cause, not an empty string"
            ),
            other => panic!("expected a failure with no gateway configured, got {other:?}"),
        }
        assert!(report.is_failure());
    });
}

#[test]
fn a_report_is_produced_for_every_host_that_was_selected_and_no_others() {
    let rt = runtime();
    in_sandbox(|| {
        let ctx = context();
        let reports = rt
            .block_on(enrol_hosts(
                &ctx,
                &Selection::All,
                &ModelProtocolOverrides::new(),
                Some(Vec::new()),
            ))
            .expect("enrol all");

        let mut ids: Vec<&str> = reports.iter().map(|r| r.host_id.as_str()).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(before, ids.len(), "each host is reported exactly once");

        for report in &reports {
            assert!(!report.display_name.is_empty());
            assert!(!report.install_action_label.is_empty());
        }
    });
}
