// `pick_target` is the pure choice-of-org-dir helper behind
// `resolve_target()`. These tests pin the precedence: the personal-session
// zero UUID wins, then mtime as a last resort. Pure helper — no real
// filesystem layout is staged, but candidate dirs must contain a
// `cowork_plugins/` subdir to be considered usable.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use systemprompt_bridge::integration::cowork_plugins::pick_target;

const PERSONAL: &str = "00000000-0000-0000-0000-000000000000";
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
fn personal_session_beats_newer_mtime() {
    let s = Sessions::new();
    let personal = s.add("acct", PERSONAL, true);
    let real = s.add("acct", REAL_ORG, true);

    let candidates = vec![(t(1_000), personal.clone()), (t(9_000), real)];

    let picked = pick_target(&candidates);
    assert_eq!(picked.as_deref(), Some(personal.as_path()));
}

#[test]
fn no_personal_falls_back_to_newest_mtime() {
    let s = Sessions::new();
    let older = s.add("acct", REAL_ORG, true);
    let newer = s.add("acct", OTHER_ORG, true);

    let candidates = vec![(t(1_000), older), (t(9_000), newer.clone())];

    let picked = pick_target(&candidates);
    assert_eq!(picked.as_deref(), Some(newer.as_path()));
}

#[test]
fn empty_candidates_returns_none() {
    let picked = pick_target(&[]);
    assert!(picked.is_none());
}

#[test]
fn half_initialised_personal_dir_falls_through_to_mtime() {
    let s = Sessions::new();
    let half_init = s.add("acct", PERSONAL, false);
    let real = s.add("acct", REAL_ORG, true);

    let candidates = vec![(t(2_000), half_init), (t(1_000), real.clone())];

    let picked = pick_target(&candidates);
    assert_eq!(picked.as_deref(), Some(real.as_path()));
}
