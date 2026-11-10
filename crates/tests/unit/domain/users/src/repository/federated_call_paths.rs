//! Every federated sign-in path goes through `resolve_federated`: the
//! production tree is scanned for its callers, for writes to
//! `federated_identities`, and for the removed linking entry points.

use std::fs;
use std::path::{Path, PathBuf};

const CALL_PATHS: [&str; 3] = [
    "crates/app/oauth-issuance/src/sso/resolve.rs",
    "crates/domain/oauth/src/services/ema.rs",
    "crates/entry/api/src/routes/messaging/identity.rs",
];

const OWNER: &str = "crates/domain/users/src/";
const DECLARATION: &str = "crates/shared/traits/src/auth.rs";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(5)
        .expect("repository root")
        .to_path_buf()
}

fn production_sources(root: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read dir") {
            let path = entry.expect("dir entry").path();
            let rel = path
                .strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                if rel != "crates/tests" && !rel.ends_with("/target") {
                    stack.push(path);
                }
            } else if rel.ends_with(".rs") {
                found.push((rel, fs::read_to_string(&path).expect("read source")));
            }
        }
    }
    found
}

#[test]
fn the_three_sign_in_paths_are_the_only_callers_outside_the_users_crate() {
    let root = repo_root();
    let mut callers: Vec<String> = production_sources(&root)
        .into_iter()
        .filter(|(rel, body)| !rel.starts_with(OWNER) && rel != DECLARATION && body.contains("resolve_federated("))
        .map(|(rel, _)| rel)
        .collect();
    callers.sort();

    assert_eq!(callers, CALL_PATHS);
}

#[test]
fn each_sign_in_path_passes_its_own_link_policy() {
    let root = repo_root();
    for (path, policy) in CALL_PATHS.iter().zip([
        "provider.policy().link_policy()",
        "&principal.link",
        "policy: &LinkPolicy",
    ]) {
        let body = fs::read_to_string(root.join(path)).expect("read call path");
        assert!(body.contains(policy), "{path} does not pass `{policy}`");
    }
}

#[test]
fn only_the_federated_repository_writes_links() {
    let root = repo_root();
    let writers: Vec<String> = production_sources(&root)
        .into_iter()
        .filter(|(_, body)| {
            body.contains("INSERT INTO federated_identities")
                || body.contains("UPDATE federated_identities SET user_id")
        })
        .map(|(rel, _)| rel)
        .collect();

    assert!(!writers.is_empty());
    for writer in writers {
        assert!(
            writer.starts_with("crates/domain/users/src/repository/federated_identity/"),
            "{writer} writes federated links outside resolve_federated"
        );
    }
}

#[test]
fn the_unconditional_linking_entry_points_are_gone() {
    let root = repo_root();
    for (rel, body) in production_sources(&root) {
        for removed in [
            "find_or_create_federated",
            "fn link_federated",
            "link_by_verified_email",
        ] {
            assert!(!body.contains(removed), "{rel} still carries `{removed}`");
        }
    }
}
