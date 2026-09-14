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
