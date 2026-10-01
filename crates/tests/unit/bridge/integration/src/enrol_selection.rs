//! `install --host <id>` resolution: which ids are accepted, which are refused,
//! and that a sync-only agent resolves rather than erroring.

use systemprompt_bridge::integration::enrol::{Selection, resolve};

fn ids(selection: &Selection) -> Result<Vec<&'static str>, String> {
    resolve(selection).map(|targets| targets.iter().map(|t| t.id()).collect())
}

#[test]
fn a_named_local_host_resolves() {
    assert_eq!(
        ids(&Selection::Ids(vec!["opencode".to_owned()])),
        Ok(vec!["opencode"])
    );
}

#[test]
fn a_sync_only_agent_resolves_rather_than_erroring() {
    // Why: install.sh passes `claude-code,opencode` on Linux. Claude Code is
    // governed through the gateway and has no profile to write, but rejecting
    // the id would fail the whole line and leave OpenCode unenrolled too.
    assert_eq!(
        ids(&Selection::Ids(vec![
            "claude-code".to_owned(),
            "opencode".to_owned()
        ])),
        Ok(vec!["claude-code", "opencode"])
    );
}

#[test]
fn order_is_the_order_the_caller_named() {
    assert_eq!(
        ids(&Selection::Ids(vec![
            "opencode".to_owned(),
            "claude-code".to_owned()
        ])),
        Ok(vec!["opencode", "claude-code"])
    );
}

#[test]
fn an_unknown_id_fails_the_whole_request() {
    let err = ids(&Selection::Ids(vec![
        "opencode".to_owned(),
        "opencodee".to_owned(),
    ]))
    .expect_err("a typo must not be silently skipped");
    assert!(err.contains("opencodee"), "{err}");
    assert!(
        err.contains("known ids"),
        "the error has to say what is valid: {err}"
    );
}

fn with_claude_cli<R>(installed: bool, body: impl FnOnce() -> R) -> R {
    let home = tempfile::tempdir().expect("home");
    let bin = tempfile::tempdir().expect("empty PATH dir");
    if installed {
        std::fs::create_dir_all(home.path().join(".claude")).expect(".claude");
    }
    temp_env::with_vars(
        [
            ("HOME", Some(home.path().as_os_str())),
            ("PATH", Some(bin.path().as_os_str())),
        ],
        body,
    )
}

#[test]
fn all_resolves_to_every_locally_installable_host() {
    let all = with_claude_cli(false, || ids(&Selection::All)).expect("all");
    assert!(all.contains(&"opencode"), "{all:?}");
    assert!(
        !all.contains(&"claude-code"),
        "with no Claude Code CLI on this machine `all` must not claim to enrol it: {all:?}"
    );
}

#[test]
fn all_includes_claude_code_once_its_cli_is_installed() {
    let all = with_claude_cli(true, || ids(&Selection::All)).expect("all");
    assert!(all.contains(&"opencode"), "{all:?}");
    assert_eq!(
        all.iter().filter(|id| **id == "claude-code").count(),
        1,
        "an installed Claude Code CLI is enrolled by `all`, exactly once: {all:?}"
    );
    assert_eq!(all.last(), Some(&"claude-code"), "it follows the host apps");
}
