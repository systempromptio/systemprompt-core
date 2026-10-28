use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use systemprompt_bridge::drive::{ConflictNotice, DriveState, OpenProposal};
use systemprompt_identifiers::{DocumentId, DocumentProposalId, DocumentRevisionId, UserId};
use tempfile::TempDir;

fn bridge_bin() -> Option<PathBuf> {
    let candidate = match std::env::var_os("SP_BRIDGE_BIN") {
        Some(explicit) => PathBuf::from(explicit),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(5)
            .expect("the test crate sits five levels below the repository root")
            .join("bin")
            .join("bridge")
            .join("target")
            .join("debug")
            .join("systemprompt-bridge"),
    };
    if candidate.is_file() {
        return Some(candidate);
    }
    assert!(
        std::env::var_os("CI").is_none(),
        "bridge binary {} unavailable under CI; scripts/test-shard.sh must prebuild it",
        candidate.display()
    );
    None
}

struct Home {
    dir: TempDir,
}

impl Home {
    fn new() -> Self {
        Self {
            dir: TempDir::new().expect("home"),
        }
    }

    fn drive(&self) -> PathBuf {
        self.dir.path().join("Systemprompt").join("Drive")
    }

    fn gateway(&self, url: &str) {
        let dir = self.dir.path().join(".config").join("systemprompt");
        std::fs::create_dir_all(&dir).expect("config dir");
        std::fs::write(
            dir.join("systemprompt-bridge.toml"),
            format!("gateway_url = \"{url}\"\n"),
        )
        .expect("config");
    }

    fn run(&self, args: &[&str]) -> Option<Output> {
        let bin = bridge_bin()?;
        let root = self.dir.path();
        let mut cmd = Command::new(bin);
        cmd.arg("drive").args(args);
        cmd.env_remove("SP_BRIDGE_PAT");
        cmd.env_remove("SP_BRIDGE_CONFIG");
        cmd.env_remove("USERPROFILE");
        cmd.env("HOME", root);
        cmd.env("XDG_CONFIG_HOME", root.join(".config"));
        cmd.env("XDG_CACHE_HOME", root.join(".cache"));
        cmd.env("XDG_DATA_HOME", root.join(".data"));
        cmd.env("XDG_STATE_HOME", root.join(".state"));
        Some(cmd.output().expect("spawn bridge"))
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn seed(drive: &Path) -> (DocumentProposalId, ConflictNotice, DocumentId) {
    let unpathed = DocumentId::generate();
    std::fs::create_dir_all(drive).expect("drive");
    std::fs::write(drive.join("plan.md"), b"head").expect("file");
    std::fs::write(drive.join("plan (conflict).md"), b"mine").expect("sibling");
    std::fs::write(drive.join("draft.md"), b"unsynced").expect("draft");
    let conflict = ConflictNotice {
        path: "plan.md".to_owned(),
        sibling: "plan (conflict).md".to_owned(),
        head_revision_id: DocumentRevisionId::generate(),
        detected_at: chrono::Utc::now(),
    };
    let proposal = DocumentProposalId::generate();
    let state = DriveState {
        cursor: Some("c1".to_owned()),
        conflicts: vec![conflict.clone()],
        proposals: vec![
            OpenProposal {
                proposal_id: proposal.clone(),
                document_id: DocumentId::generate(),
                path: Some("plan.md".to_owned()),
                proposer_id: UserId::new("agent-user"),
                message: Some("tighten intro".to_owned()),
                created_at: chrono::Utc::now(),
            },
            OpenProposal {
                proposal_id: DocumentProposalId::generate(),
                document_id: unpathed.clone(),
                path: None,
                proposer_id: UserId::new("bob"),
                message: None,
                created_at: chrono::Utc::now(),
            },
        ],
        ..DriveState::default()
    };
    state.save(&drive.join(".drive-state.json")).expect("state");
    (proposal, conflict, unpathed)
}

#[test]
fn status_without_a_folder_says_drive_is_not_set_up() {
    let home = Home::new();
    let Some(out) = home.run(&[]) else { return };
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("drive folder:"), "{text}");
    assert!(text.contains("not set up"), "{text}");
}

#[test]
fn status_lists_pending_uploads_conflicts_and_proposals() {
    let home = Home::new();
    let (proposal, _, unpathed) = seed(&home.drive());
    let Some(out) = home.run(&["status"]) else {
        return;
    };
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("conflicts: 1"), "{text}");
    assert!(text.contains("open proposals: 2"), "{text}");
    assert!(text.contains("pending uploads:"), "{text}");
    assert!(
        text.contains(&format!(
            "{proposal}  plan.md  by agent-user: tighten intro"
        )),
        "{text}"
    );
    assert!(text.contains(&format!("{unpathed}  by bob")), "{text}");
}

#[test]
fn conflicts_lists_each_kept_local_edit() {
    let home = Home::new();
    let (_, conflict, _) = seed(&home.drive());
    let Some(out) = home.run(&["conflicts"]) else {
        return;
    };
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains(&format!(
            "plan.md  kept local edit as plan (conflict).md  (head {}",
            conflict.head_revision_id
        )),
        "{text}"
    );
}

#[test]
fn conflicts_with_none_open_says_so() {
    let home = Home::new();
    std::fs::create_dir_all(home.drive()).expect("drive");
    let Some(out) = home.run(&["conflicts"]) else {
        return;
    };
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("no drive conflicts"));
}

#[test]
fn a_corrupt_state_file_fails_once_and_is_quarantined() {
    let home = Home::new();
    std::fs::create_dir_all(home.drive()).expect("drive");
    std::fs::write(home.drive().join(".drive-state.json"), b"{not json").expect("state");
    let Some(out) = home.run(&["conflicts"]) else {
        return;
    };
    assert_eq!(out.status.code(), Some(1));
    assert!(stderr(&out).contains("corrupt"), "{}", stderr(&out));
    let quarantined = std::fs::read_dir(home.drive())
        .expect("drive")
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().contains(".corrupt-"));
    assert!(quarantined);
    let Some(again) = home.run(&["status"]) else {
        return;
    };
    assert!(again.status.success(), "{}", stderr(&again));
}

#[test]
fn an_unknown_subcommand_is_a_usage_error() {
    let home = Home::new();
    let Some(out) = home.run(&["frobnicate"]) else {
        return;
    };
    assert_eq!(out.status.code(), Some(64));
    assert!(
        stderr(&out).contains("unknown subcommand"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn sync_without_a_credential_fails_with_the_reason() {
    let home = Home::new();
    home.gateway("http://127.0.0.1:9");
    let Some(out) = home.run(&["sync"]) else {
        return;
    };
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert!(
        stderr(&out).contains("drive sync failed"),
        "{}",
        stderr(&out)
    );
}
