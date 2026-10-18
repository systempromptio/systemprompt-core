#![cfg(unix)]
use std::path::Path;
use std::process::{Command, Stdio};

use systemprompt_marketplace::managed::{
    GitCaptureRequest, GitSourceCapture, ManagedError, NativeGitSourceCapture,
};

use crate::git_https::Fixture;

fn run(directory: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(directory)
        .args([
            "-c",
            "user.name=fixture",
            "-c",
            "user.email=fixture@example.test",
        ])
        .args(args)
        .stderr(Stdio::null())
        .output()
        .expect("git installed");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

fn publish(fixture: &Fixture, branch: &str, build: impl FnOnce(&Path)) -> String {
    let work = tempfile::tempdir().expect("work tree");
    run(work.path(), &["init", "-q"]);
    build(work.path());
    run(work.path(), &["add", "-A"]);
    run(work.path(), &["commit", "-q", "-m", branch]);
    let bare = fixture.directory.path().join("repo.git");
    let target = format!("HEAD:refs/heads/{branch}");
    run(
        work.path(),
        &["push", "-q", bare.to_str().expect("utf8 path"), &target],
    );
    run(&bare, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    run(work.path(), &["rev-parse", "HEAD"])
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("directories");
    std::fs::write(path, contents).expect("fixture file");
}

fn skill_tree(root: &Path) {
    write(root, "README.md", "outside the root");
    write(
        root,
        "plugins/skill-a/SKILL.md",
        "---\nname: skill-a\n---\n",
    );
    write(root, "plugins/skill-a/bin/run.sh", "#!/bin/sh\n");
    let script = root.join("plugins/skill-a/bin/run.sh");
    let mut permissions = std::fs::metadata(&script).expect("script").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(script, permissions).expect("executable bit");
}

struct Harness {
    fixture: Fixture,
    scratch: tempfile::TempDir,
    url: String,
}

impl Harness {
    fn start() -> Self {
        let fixture = Fixture::start();
        let url = format!("https://127.0.0.1:{}/repo.git", fixture.port);
        Self {
            fixture,
            scratch: tempfile::tempdir().expect("scratch root"),
            url,
        }
    }

    fn trusted(&self) -> NativeGitSourceCapture {
        let pem = std::fs::read(self.fixture.directory.path().join("cert.pem")).expect("ca");
        NativeGitSourceCapture::new(self.scratch.path().to_path_buf())
            .with_certificate_authority(pem)
    }

    fn request<'a>(&'a self, reference: &'a str, root: &'a str) -> GitCaptureRequest<'a> {
        GitCaptureRequest {
            repository: &self.url,
            reference,
            subdirectory: Some("plugins"),
            root,
            credential: Some("first-token"),
        }
    }

    fn scratch_is_empty(&self) -> bool {
        std::fs::read_dir(self.scratch.path())
            .expect("scratch listing")
            .next()
            .is_none()
    }
}

#[test]
fn branch_capture_imports_only_the_rooted_regular_files() {
    let harness = Harness::start();
    let commit = publish(&harness.fixture, "main", skill_tree);

    let captured = harness
        .trusted()
        .capture(&harness.request("main", "skill-a"))
        .expect("captured source");

    assert_eq!(captured.commit, commit);
    let paths: Vec<&str> = captured.files.0.keys().map(String::as_str).collect();
    assert_eq!(paths, ["SKILL.md", "bin/run.sh"]);
    assert_eq!(
        captured.files.0["SKILL.md"].bytes,
        b"---\nname: skill-a\n---\n"
    );
    assert!(!captured.files.0["SKILL.md"].executable);
    assert!(captured.files.0["bin/run.sh"].executable);
    assert!(harness.scratch_is_empty(), "checkout removed after capture");
}

#[test]
fn pinned_commit_capture_skips_ref_resolution() {
    let harness = Harness::start();
    let commit = publish(&harness.fixture, "main", skill_tree);

    let captured = harness
        .trusted()
        .capture(&harness.request(&commit, "skill-a"))
        .expect("captured pinned commit");

    assert_eq!(captured.commit, commit);
    assert_eq!(captured.files.0.len(), 2);
}

#[test]
fn absent_root_captures_an_empty_tree() {
    let harness = Harness::start();
    publish(&harness.fixture, "main", skill_tree);

    let captured = harness
        .trusted()
        .capture(&harness.request("main", "missing"))
        .expect("empty capture");

    assert!(captured.files.0.is_empty());
}

#[test]
fn unknown_reference_fails_and_leaves_no_checkout() {
    let harness = Harness::start();
    publish(&harness.fixture, "main", skill_tree);

    let error = harness
        .trusted()
        .capture(&harness.request("does-not-exist", "skill-a"))
        .expect_err("unknown ref rejected");

    assert!(matches!(error, ManagedError::Conflict(_)), "{error:?}");
    assert!(harness.scratch_is_empty());
}

#[test]
fn untrusted_certificate_is_refused_without_a_configured_authority() {
    let harness = Harness::start();
    publish(&harness.fixture, "main", skill_tree);
    let capture = NativeGitSourceCapture::new(harness.scratch.path().to_path_buf());

    let error = capture
        .capture(&harness.request("main", "skill-a"))
        .expect_err("self-signed certificate rejected");

    assert!(matches!(error, ManagedError::Conflict(_)), "{error:?}");
    assert!(harness.scratch_is_empty());
}

#[test]
fn wrong_credential_is_refused_without_echoing_the_token() {
    let harness = Harness::start();
    publish(&harness.fixture, "main", skill_tree);
    let request = GitCaptureRequest {
        credential: Some("stale-token"),
        ..harness.request("main", "skill-a")
    };

    let error = harness.trusted().capture(&request).expect_err("rejected");

    assert!(!error.to_string().contains("stale-token"));
}

#[test]
fn symbolic_links_under_the_root_are_rejected() {
    let harness = Harness::start();
    publish(&harness.fixture, "linked", |root| {
        skill_tree(root);
        std::os::unix::fs::symlink("SKILL.md", root.join("plugins/skill-a/alias.md"))
            .expect("symlink");
    });

    let error = harness
        .trusted()
        .capture(&harness.request("linked", "skill-a"))
        .expect_err("symlink rejected");

    assert!(error.to_string().contains("non-regular files"), "{error}");
    assert!(harness.scratch_is_empty());
}

#[test]
fn nested_git_metadata_anywhere_in_the_tree_is_rejected() {
    let harness = Harness::start();
    publish(&harness.fixture, "submodule", |root| {
        skill_tree(root);
        write(root, ".gitmodules", "[submodule \"x\"]\n");
    });

    let error = harness
        .trusted()
        .capture(&harness.request("submodule", "skill-a"))
        .expect_err("gitmodules rejected");

    assert!(error.to_string().contains("submodules"), "{error}");
}

#[test]
fn unportable_paths_under_the_root_are_rejected() {
    let harness = Harness::start();
    publish(&harness.fixture, "colon", |root| {
        skill_tree(root);
        write(root, "plugins/skill-a/a:b.md", "colon");
    });

    let error = harness
        .trusted()
        .capture(&harness.request("colon", "skill-a"))
        .expect_err("unportable path rejected");

    assert!(error.to_string().contains("portable"), "{error}");
}
