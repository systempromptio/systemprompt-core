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

use systemprompt_bridge::integration::cowork_plugins::{PERSONAL_SESSION_UUID, pick_target};

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

    let picked = pick_target(&candidates);
    assert_eq!(picked.as_deref(), Some(personal.as_path()));
}

#[test]
fn single_usable_org_is_selected() {
    let s = Sessions::new();
    let usable = s.add("acct", REAL_ORG, true);
    let half_init = s.add("acct", OTHER_ORG, false);

    let candidates = vec![(t(1_000), usable.clone()), (t(9_000), half_init)];

    let picked = pick_target(&candidates);
    assert_eq!(picked.as_deref(), Some(usable.as_path()));
}

#[test]
fn several_usable_orgs_without_personal_are_refused() {
    let s = Sessions::new();
    let older = s.add("acct", REAL_ORG, true);
    let newer = s.add("acct", OTHER_ORG, true);

    let candidates = vec![(t(1_000), older), (t(9_000), newer)];

    // The newer candidate is not a tie-break: mtime does not disambiguate, so
    // the operator must name the session via `cowork.session_org_dir`.
    assert!(pick_target(&candidates).is_none());
}

#[test]
fn no_usable_org_returns_none() {
    let s = Sessions::new();
    let half_init = s.add("acct", REAL_ORG, false);

    let candidates = vec![(t(1_000), half_init)];

    assert!(pick_target(&candidates).is_none());
}

#[test]
fn empty_candidates_returns_none() {
    let picked = pick_target(&[]);
    assert!(picked.is_none());
}

#[test]
fn half_initialised_personal_dir_is_skipped_for_personal_match() {
    let s = Sessions::new();
    let half_init = s.add("acct", PERSONAL, false);
    let real = s.add("acct", REAL_ORG, true);

    let candidates = vec![(t(1_000), half_init), (t(9_000), real.clone())];

    let picked = pick_target(&candidates);
    assert_eq!(picked.as_deref(), Some(real.as_path()));
}
