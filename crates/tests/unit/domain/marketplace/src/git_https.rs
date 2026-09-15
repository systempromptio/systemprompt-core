#![cfg(unix)]
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use systemprompt_marketplace::managed::git_execution::{GitExecutionLimits, execute};

struct Fixture {
    directory: tempfile::TempDir,
    server: Child,
    port: String,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.kill().expect("stop fixture HTTPS server");
        self.server.wait().expect("reap fixture HTTPS server");
    }
}

impl Fixture {
    fn start() -> Self {
        let directory = tempfile::tempdir().expect("fixture directory");
        let root = directory.path();
        let cert = Command::new("openssl")
            .current_dir(root)
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-keyout",
                "key.pem",
                "-out",
                "cert.pem",
                "-days",
                "1",
                "-subj",
                "/CN=localhost",
                "-addext",
                "subjectAltName=DNS:localhost,IP:127.0.0.1",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("openssl installed");
        assert!(cert.success());
        assert!(
            Command::new("git")
                .args(["init", "--bare"])
                .arg(root.join("repo.git"))
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("git installed")
                .success()
        );
        std::fs::write(root.join("token"), "first-token").expect("fixture credential");
        std::fs::write(root.join("server.py"), include_str!("git_https_fixture.py"))
            .expect("fixture server");
        let exec_path = Command::new("git")
            .arg("--exec-path")
            .output()
            .expect("git exec path");
        let backend = format!(
            "{}/git-http-backend",
            String::from_utf8(exec_path.stdout).expect("utf8").trim()
        );
        let mut server = Command::new("python3")
            .arg(root.join("server.py"))
            .arg(root)
            .arg(backend)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("HTTPS fixture server");
        let mut port = String::new();
        BufReader::new(server.stdout.take().expect("server port pipe"))
            .read_line(&mut port)
            .expect("server port");
        assert!(!port.trim().is_empty(), "HTTPS server started");
        Self {
            directory,
            server,
            port: port.trim().to_owned(),
        }
    }

    fn request(
        &self,
        path: &str,
        credential: &str,
    ) -> systemprompt_marketplace::managed::Result<Vec<u8>> {
        let url = format!("https://127.0.0.1:{}/{path}", self.port);
        let mut command = Command::new("git");
        command.args([
            "-c",
            &format!(
                "http.sslCAInfo={}",
                self.directory.path().join("cert.pem").display()
            ),
            "ls-remote",
            &url,
        ]);
        execute(
            &mut command,
            Some((&url, credential)),
            GitExecutionLimits::default(),
        )
    }
}

#[test]
fn authenticated_https_git_rotation_redirects_and_redacted_failures() {
    let fixture = Fixture::start();
    fixture
        .request("repo.git", "first-token")
        .expect("real authenticated smart HTTP");
    std::fs::write(fixture.directory.path().join("token"), "rotated-token")
        .expect("rotate fixture token");
    let error = fixture
        .request("repo.git", "first-token")
        .expect_err("old credential rejected");
    assert!(!error.to_string().contains("first-token"));
    assert!(!error.to_string().contains("private-auth-failure"));
    fixture
        .request("repo.git", "rotated-token")
        .expect("rotated credential accepted");
    assert!(fixture.request("redirect.git", "rotated-token").is_err());
    assert!(!fixture.directory.path().join("forwarded").exists());
}


#[test]
fn separate_https_sources_require_independent_credentials_and_rotate_without_cross_talk() {
    let root = Fixture::start();
    let dependency = Fixture::start();
    std::fs::write(
        dependency.directory.path().join("token"),
        "dependency-token",
    )
    .unwrap();
    root.request("repo.git", "first-token")
        .expect("root authenticated");
    dependency
        .request("repo.git", "dependency-token")
        .expect("dependency authenticated");
    let wrong_dependency = dependency
        .request("repo.git", "first-token")
        .expect_err("root credential cannot access dependency");
    assert!(!wrong_dependency.to_string().contains("first-token"));
    assert!(root.request("repo.git", "dependency-token").is_err());
    std::fs::write(
        dependency.directory.path().join("token"),
        "dependency-rotated",
    )
    .unwrap();
    assert!(dependency.request("repo.git", "dependency-token").is_err());
    dependency
        .request("repo.git", "dependency-rotated")
        .expect("only dependency rotated");
    root.request("repo.git", "first-token")
        .expect("independent root still authenticated");
}


impl Fixture {
    fn git_object(&self, args: &[&str], input: &str) -> String {
        use std::io::Write;
        let mut child = Command::new("git")
            .current_dir(self.directory.path().join("repo.git"))
            .args([
                "-c",
                "user.name=Feedback fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("local fixture Git");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "local fixture Git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn commit_tree(&self, extra: Option<(&str, &str)>) -> String {
        let script = self.git_object(
            &["hash-object", "-w", "--stdin"],
            "#!/bin/sh\necho verified\n",
        );
        let doc = self.git_object(&["hash-object", "-w", "--stdin"], "# Verified skill\n");
        let root = self.git_object(
            &["mktree"],
            &format!("100755 blob {script}\trun.sh\n100644 blob {doc}\tSKILL.md\n"),
        );
        let mut tree = format!("040000 tree {root}\troot\n");
        if let Some((kind, object)) = extra {
            tree.push_str(&format!("{kind} {object}\n"));
        }
        let tree = self.git_object(&["mktree"], &tree);
        let commit = self.git_object(&["commit-tree", &tree], "owned local fixture commit\n");
        self.git_object(&["update-ref", "refs/heads/main", &commit], "");
        commit
    }

    fn input(
        &self,
        commit: String,
    ) -> systemprompt_models::feedback::verification::DependencyVerificationInput {
        systemprompt_models::feedback::verification::DependencyVerificationInput {
            revision_id: systemprompt_identifiers::ResourceRevisionId::new("fixture-revision"),
            source_id: systemprompt_identifiers::ManagedSourceId::new("fixture-source"),
            exact_commit: commit,
            relative_root: "root".to_owned(),
            dependencies: Vec::new(),
        }
    }

    fn tree_reader(&self) -> impl systemprompt_marketplace::managed::GitTreeReader {
        systemprompt_marketplace::managed::NativeGitTreeReader::with_certificate_authority(
            &self.directory.path().join("cert.pem"),
        )
        .expect("explicit fixture CA")
    }

    fn tree_url(&self) -> String {
        format!("https://127.0.0.1:{}/repo.git", self.port)
    }
}

#[test]
fn native_https_fetch_reads_exact_commit_relative_root_bytes_and_executable_modes() {
    use systemprompt_marketplace::managed::{GitTreeRead, GitTreeReader};
    let f = Fixture::start();
    let input = f.input(f.commit_tree(None));
    let reader = f.tree_reader();
    let deadline = || std::time::Instant::now() + std::time::Duration::from_secs(20);
    let files = reader
        .read(&GitTreeRead {
            input: &input,
            repository: &f.tree_url(),
            subdirectory: None,
            credential: Some("first-token"),
            deadline: deadline(),
        })
        .expect("real authenticated fetch and retained tree read");
    assert_eq!(files.0.len(), 2);
    assert_eq!(files.0["run.sh"].bytes, b"#!/bin/sh\necho verified\n");
    assert!(files.0["run.sh"].executable);
    assert_eq!(files.0["SKILL.md"].bytes, b"# Verified skill\n");
    assert!(!files.0["SKILL.md"].executable);
    assert!(
        reader
            .read(&GitTreeRead {
                input: &input,
                repository: &f.tree_url(),
                subdirectory: None,
                credential: Some("wrong-token"),
                deadline: deadline(),
            })
            .is_err()
    );
    let mut wrong_commit = input.clone();
    wrong_commit.exact_commit = "b".repeat(40);
    assert!(
        reader
            .read(&GitTreeRead {
                input: &wrong_commit,
                repository: &f.tree_url(),
                subdirectory: None,
                credential: Some("first-token"),
                deadline: deadline(),
            })
            .is_err()
    );
    let mut absent_root = input.clone();
    absent_root.relative_root = "absent".to_owned();
    assert!(
        reader
            .read(&GitTreeRead {
                input: &absent_root,
                repository: &f.tree_url(),
                subdirectory: None,
                credential: Some("first-token"),
                deadline: deadline(),
            })
            .expect("absent root is empty and cannot match retained revision")
            .0
            .is_empty()
    );
    assert!(
        reader
            .read(&GitTreeRead {
                input: &input,
                repository: &f.tree_url(),
                subdirectory: Some("wrong-prefix"),
                credential: Some("first-token"),
                deadline: deadline(),
            })
            .expect("wrong registered subdirectory cannot expose repository root")
            .0
            .is_empty()
    );
    assert!(
        reader
            .read(&GitTreeRead {
                input: &input,
                repository: &f.tree_url(),
                subdirectory: None,
                credential: Some("first-token"),
                deadline: std::time::Instant::now(),
            })
            .is_err()
    );
    // Failed fetch and expired deadline clean up before the following valid
    // operation.
    reader
        .read(&GitTreeRead {
            input: &input,
            repository: &f.tree_url(),
            subdirectory: None,
            credential: Some("first-token"),
            deadline: deadline(),
        })
        .expect("recovery after rejected fetches");
}

#[test]
fn native_fetch_rejects_submodules_gitmodules_and_nested_git_metadata_outside_selected_root() {
    use systemprompt_marketplace::managed::{GitTreeRead, GitTreeReader};
    let f = Fixture::start();
    let valid_commit = f.commit_tree(None);
    let metadata = f.git_object(
        &["hash-object", "-w", "--stdin"],
        "forbidden repository metadata",
    );
    let nested = f.git_object(&["mktree"], &format!("100644 blob {metadata}\t.git\n"));
    for (kind, object) in [
        ("100644 blob", format!("{metadata}\t.gitmodules")),
        (
            "160000 commit",
            format!("{valid_commit}\tundeclared-dependency"),
        ),
        ("040000 tree", format!("{nested}\tnested-repository")),
    ] {
        let input = f.input(f.commit_tree(Some((kind, &object))));
        let error = f
            .tree_reader()
            .read(&GitTreeRead {
                input: &input,
                repository: &f.tree_url(),
                subdirectory: None,
                credential: Some("first-token"),
                deadline: std::time::Instant::now() + std::time::Duration::from_secs(20),
            })
            .expect_err("undeclared repository metadata denied across entire fetched tree");
        assert!(!error.to_string().contains("first-token"));
    }
}

#[test]
fn explicit_certificate_authority_is_bounded_regular_and_copied_without_ambient_trust_changes() {
    use std::os::unix::fs::symlink;
    use systemprompt_marketplace::managed::{GitTreeRead, GitTreeReader, NativeGitTreeReader};
    let f = Fixture::start();
    let input = f.input(f.commit_tree(None));
    let certificate = f.directory.path().join("cert.pem");
    let link = f.directory.path().join("linked-ca");
    symlink(&certificate, &link).unwrap();
    assert!(NativeGitTreeReader::with_certificate_authority(&link).is_err());
    assert!(NativeGitTreeReader::with_certificate_authority(f.directory.path()).is_err());
    let empty = f.directory.path().join("empty-ca");
    std::fs::File::create(&empty).unwrap();
    assert!(NativeGitTreeReader::with_certificate_authority(&empty).is_err());
    let fifo = f.directory.path().join("fifo-ca");
    assert!(
        Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("owned FIFO fixture")
            .success()
    );
    let started = std::time::Instant::now();
    assert!(NativeGitTreeReader::with_certificate_authority(&fifo).is_err());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "nonregular CA must not block waiting for a writer"
    );
    let oversized = f.directory.path().join("oversized-ca");
    std::fs::File::create(&oversized)
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    assert!(NativeGitTreeReader::with_certificate_authority(&oversized).is_err());
    let reader = NativeGitTreeReader::with_certificate_authority(&certificate).unwrap();
    std::fs::write(&certificate, "changed after construction").unwrap();
    reader
        .read(&GitTreeRead {
            input: &input,
            repository: &f.tree_url(),
            subdirectory: None,
            credential: Some("first-token"),
            deadline: std::time::Instant::now() + std::time::Duration::from_secs(20),
        })
        .expect("reader retains its own CA bytes");
    assert!(
        NativeGitTreeReader
            .read(&GitTreeRead {
                input: &input,
                repository: &f.tree_url(),
                subdirectory: None,
                credential: Some("first-token"),
                deadline: std::time::Instant::now() + std::time::Duration::from_secs(20),
            })
            .is_err()
    );
}
