// `pick_target` is the pure choice-of-org-dir helper behind
// `resolve_target()`. These tests pin the precedence: the personal-session
// zero UUID wins, and otherwise a target is chosen only when exactly one
// candidate is usable — an ambiguous set is refused rather than guessed, since
// picking the wrong Cowork org session writes plugins into the wrong tenant.
// Pure helper — no real filesystem layout is staged, but candidate dirs must
// contain a `cowork_plugins/` subdir to be considered usable.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use systemprompt_bridge::cowork_compat::PERSONAL_SESSION_UUID;
use systemprompt_bridge::integration::cowork_plugins::{
    ResolveTargetError, TargetSource, pick_target,
};

const PERSONAL: &str = PERSONAL_SESSION_UUID;
const REAL_ORG: &str = "f8e4d915-1111-2222-3333-444444444444";
const OTHER_ORG: &str = "a1b2c3d4-5555-6666-7777-888888888888";

struct Sessions {
    root: tempfile::TempDir,
}

impl Sessions {
    fn new() -> Self {
        Self {
            root: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn add(&self, account: &str, org: &str, with_cowork_plugins: bool) -> PathBuf {
        let path = self.root.path().join(account).join(org);
        fs::create_dir_all(&path).expect("mkdir org");
        if with_cowork_plugins {
            fs::create_dir_all(path.join("cowork_plugins")).expect("mkdir cowork_plugins");
        }
        path
    }
}

fn t(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

#[test]
fn personal_session_wins_over_another_usable_org() {
    let s = Sessions::new();
    let personal = s.add("acct", PERSONAL, true);
    let real = s.add("acct", REAL_ORG, true);

    let candidates = vec![(t(1_000), personal.clone()), (t(9_000), real)];

    let picked = pick_target(&candidates).expect("a personal session resolves");
    assert_eq!(picked, Some((personal, TargetSource::Personal)));
}

#[test]
fn single_usable_org_is_selected() {
    let s = Sessions::new();
    let usable = s.add("acct", REAL_ORG, true);
    let half_init = s.add("acct", OTHER_ORG, false);

    let candidates = vec![(t(1_000), usable.clone()), (t(9_000), half_init)];

    let picked = pick_target(&candidates).expect("one usable session resolves");
    assert_eq!(picked, Some((usable, TargetSource::OnlySession)));
}

#[test]
fn several_usable_orgs_without_personal_are_ambiguous() {
    let s = Sessions::new();
    let older = s.add("acct", REAL_ORG, true);
    let newer = s.add("acct", OTHER_ORG, true);

    let candidates = vec![(t(1_000), older), (t(9_000), newer)];

    // The newer candidate is not a tie-break: mtime does not disambiguate, so
    // the operator must name the session via `cowork.session_org_dir`.
    assert!(matches!(
        pick_target(&candidates),
        Err(ResolveTargetError::Ambiguous { .. })
    ));
}

#[test]
fn no_usable_org_returns_none() {
    let s = Sessions::new();
    let half_init = s.add("acct", REAL_ORG, false);

    let candidates = vec![(t(1_000), half_init)];

    assert!(
        pick_target(&candidates)
            .expect("no usable dir is not an error")
            .is_none()
    );
}

#[test]
fn empty_candidates_returns_none() {
    let picked = pick_target(&[]).expect("no candidates is not an error");
    assert!(picked.is_none());
}

#[test]
fn half_initialised_personal_dir_is_skipped_for_personal_match() {
    let s = Sessions::new();
    let half_init = s.add("acct", PERSONAL, false);
    let real = s.add("acct", REAL_ORG, true);

    let candidates = vec![(t(1_000), half_init), (t(9_000), real.clone())];

    let picked = pick_target(&candidates).expect("the real org resolves");
    assert_eq!(picked, Some((real, TargetSource::OnlySession)));
}

#[test]
fn a_personal_session_named_by_its_prefix_wins_among_several() {
    let s = Sessions::new();
    let prefix = &PERSONAL[..8];
    let personal = s.add("acct", prefix, true);
    let real = s.add("acct", REAL_ORG, true);
    let other = s.add("acct", OTHER_ORG, true);

    let candidates = vec![
        (t(9_000), real),
        (t(1_000), personal.clone()),
        (t(5_000), other),
    ];

    let picked =
        pick_target(&candidates).expect("Desktop's prefix-named personal session resolves");
    assert_eq!(picked, Some((personal, TargetSource::Personal)));
}

#[test]
fn a_prefix_shorter_than_eight_characters_is_not_the_personal_session() {
    let s = Sessions::new();
    let short = s.add("acct", &PERSONAL[..4], true);
    let real = s.add("acct", REAL_ORG, true);
    let candidates = vec![(t(1_000), short), (t(2_000), real)];
    assert!(matches!(
        pick_target(&candidates),
        Err(ResolveTargetError::Ambiguous { .. })
    ));
}

#[test]
fn a_clear_visits_every_usable_session_when_the_choice_is_ambiguous() {
    let home = tempfile::tempdir().expect("home");
    let sessions = home
        .path()
        .join("Claude-3p")
        .join("local-agent-mode-sessions")
        .join("acct");
    for org in [REAL_ORG, OTHER_ORG] {
        fs::create_dir_all(sessions.join(org).join("cowork_plugins")).expect("session");
    }
    let vars = vec![
        ("XDG_CONFIG_HOME", Some(home.path().display().to_string())),
        ("HOME", Some(home.path().display().to_string())),
        ("SP_BRIDGE_CONFIG", None),
    ];
    temp_env::with_vars(vars, || {
        assert!(matches!(
            systemprompt_bridge::integration::cowork_plugins::resolve_target(),
            Err(ResolveTargetError::Ambiguous { .. })
        ));
        let mut targets: Vec<PathBuf> =
            systemprompt_bridge::integration::cowork_plugins::clear_targets()
                .expect("an ambiguous tree still yields every session to clear")
                .into_iter()
                .map(|t| t.session_org_dir)
                .collect();
        targets.sort();
        let mut expected = vec![sessions.join(OTHER_ORG), sessions.join(REAL_ORG)];
        expected.sort();
        assert_eq!(targets, expected);
    });
}
