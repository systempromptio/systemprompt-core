//! Cloud initialization keeps cloned content while removing repository
//! metadata.

#![cfg(unix)]

use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use systemprompt_cli::cloud::{self, CloudCommands};

use super::json_ctx;

const HELPER: &str = "cloud_harness::init_clone_lifecycle::successful_admin_clone_helper";
const REPOSITORY: &str = "https://github.com/systempromptio/systemprompt-mcp-admin.git";

struct OwnedChild(Option<Child>);

impl OwnedChild {
    const fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.0.as_mut().expect("owned child present").try_wait()
    }

    fn disarm(&mut self) {
        self.0.take();
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct CurrentDirectory(std::path::PathBuf);

impl Drop for CurrentDirectory {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

#[tokio::test]
#[ignore = "re-executed by successful_admin_clone_keeps_files_and_removes_git_metadata"]
async fn successful_admin_clone_helper() {
    let root = tempfile::tempdir().expect("owned init root");
    let tools = tempfile::tempdir().expect("owned tool directory");
    let calls = root.path().join("git-calls");
    let git = tools.path().join("git");
    std::fs::write(
        &git,
        format!(
            "#!/bin/sh\nprintf '%s|%s\\n' \"$PWD\" \"$*\" >> '{}'\nfor arg in \"$@\"; do target=\"$arg\"; done\nmkdir -p \"$target/.git\"\nprintf 'owned clone payload\\n' > \"$target/README.fixture\"\nprintf 'owned metadata\\n' > \"$target/.git/config\"\n",
            calls.display()
        ),
    )
    .expect("write owned git shim");
    let mut permissions = std::fs::metadata(&git)
        .expect("git shim metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&git, permissions).expect("make git shim executable");

    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![tools.path().to_path_buf()];
    paths.extend(std::env::split_paths(&old_path));
    unsafe {
        std::env::set_var(
            "PATH",
            std::env::join_paths(paths).expect("owned PATH value"),
        );
    }
    let previous = std::env::current_dir().expect("current directory");
    let _cwd = CurrentDirectory(previous);
    std::env::set_current_dir(root.path()).expect("enter owned project");

    cloud::execute(CloudCommands::Init { force: false }, &json_ctx())
        .await
        .expect("initialize project through public cloud command");

    let cloned = root.path().join("services/mcp/systemprompt-admin");
    assert_eq!(
        std::fs::read_to_string(cloned.join("README.fixture")).expect("cloned payload"),
        "owned clone payload\n"
    );
    assert!(
        !cloned.join(".git").exists(),
        "generated project must not retain the upstream repository metadata"
    );
    assert!(root.path().join("services/config/config.yaml").is_file());
    assert!(root.path().join(".systemprompt/Dockerfile").is_file());

    let expected_target = cloned.display().to_string();
    let invocation = std::fs::read_to_string(&calls).expect("recorded git invocation");
    assert_eq!(
        invocation,
        format!(
            "{}|clone --depth 1 {} {}\n",
            root.path().display(),
            REPOSITORY,
            expected_target
        )
    );

    cloud::execute(CloudCommands::Init { force: false }, &json_ctx())
        .await
        .expect("rerun initialized project");
    assert_eq!(
        std::fs::read_to_string(&calls).expect("git invocation remains recorded"),
        invocation,
        "an initialized services tree must not be cloned again"
    );
    assert_eq!(
        std::fs::read_to_string(cloned.join("README.fixture")).expect("preserved payload"),
        "owned clone payload\n"
    );
}

fn bounded_helper(helper: &str) -> (String, String) {
    let stdout = tempfile::tempfile().expect("owned helper stdout");
    let stderr = tempfile::tempfile().expect("owned helper stderr");
    let child = Command::new(std::env::current_exe().expect("cloud harness binary"))
        .args(["--exact", helper, "--ignored", "--nocapture"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().expect("clone stdout")))
        .stderr(Stdio::from(stderr.try_clone().expect("clone stderr")))
        .spawn()
        .expect("spawn isolated init helper");
    let mut child = OwnedChild::new(child);
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll init helper") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "init helper exceeded twenty seconds"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    child.disarm();
    let read = |mut file: std::fs::File| {
        file.seek(SeekFrom::Start(0)).expect("rewind capture");
        let mut text = String::new();
        file.read_to_string(&mut text).expect("read capture");
        text
    };
    let stdout = read(stdout);
    let stderr = read(stderr);
    assert!(
        status.success(),
        "isolated cloud-init helper failed; stdout={stdout}; stderr={stderr}"
    );
    (stdout, stderr)
}

#[test]
fn successful_admin_clone_keeps_files_and_removes_git_metadata() {
    let _ = bounded_helper(HELPER);
}
