use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus};
use std::time::Duration;

use systemprompt_device::platform::instance::{
    Acquisition, HandOff, InstanceError, InstanceGuard, RunningInstance, acquire,
};
use tempfile::TempDir;
use tokio::time::{Instant, sleep, timeout};

// Why: the primary keeps listening after a hand-off it rejected, so an
// unbounded accept would hide the launch's own error behind a hung test.
const ROUND: Duration = Duration::from_secs(20);

// Why: the second launch runs in a process of its own, this test binary
// started again on the helper below, so the exchange crosses a real process
// boundary the way a second start of the app does.
const HELPER: &str = "platform::instance_tests::second_launch_process";
const HELPER_ROOT: &str = "SYSTEMPROMPT_INSTANCE_HELPER_ROOT";
const HELPER_EXPECT: &str = "SYSTEMPROMPT_INSTANCE_HELPER_EXPECT";
const EXPECT_DELIVERED: &str = "delivered";
const EXPECT_REFUSED: &str = "refused";
const HELPER_ARGUMENT: &str = "sp-harness://settings?page=about";

// Why: the launch's own waits end within seconds; this bound only keeps a
// regression from hanging the run.
const PROCESS_EXIT: Duration = Duration::from_secs(15);

const EXIT_POLL: Duration = Duration::from_millis(20);

fn spawn_second_launch(root: &TempDir, expect: &str) -> Child {
    Command::new(std::env::current_exe().expect("test binary path"))
        .args([HELPER, "--exact", "--nocapture"])
        .env(HELPER_ROOT, root.path())
        .env(HELPER_EXPECT, expect)
        .spawn()
        .expect("start the second launch")
}

async fn exit_of(mut child: Child) -> ExitStatus {
    let deadline = Instant::now() + PROCESS_EXIT;
    loop {
        if let Some(status) = child.try_wait().expect("poll the second launch") {
            return status;
        }
        if Instant::now() >= deadline {
            child.kill().expect("stop the hung second launch");
            panic!("the second launch did not exit within {PROCESS_EXIT:?}");
        }
        sleep(EXIT_POLL).await;
    }
}

#[tokio::test]
async fn second_launch_process() {
    let Some(root) = std::env::var_os(HELPER_ROOT).map(PathBuf::from) else {
        return;
    };
    let expect = std::env::var(HELPER_EXPECT).expect("the helper names its outcome");
    let running = match acquire(&root).expect("acquire the held data root") {
        Acquisition::Secondary(running) => running,
        Acquisition::Primary(_) => panic!("the data root must be held by the first process"),
    };
    let outcome = running
        .hand_off(&HandOff {
            arguments: vec![HELPER_ARGUMENT.to_owned()],
            working_directory: None,
        })
        .await;
    match expect.as_str() {
        EXPECT_DELIVERED => outcome.expect("the running instance acknowledges"),
        EXPECT_REFUSED => assert!(outcome.is_err(), "nothing accepted, got {outcome:?}"),
        other => panic!("unknown helper outcome {other}"),
    }
}

fn primary(root: &TempDir) -> InstanceGuard {
    match acquire(root.path()).expect("acquire a free data root") {
        Acquisition::Primary(guard) => guard,
        Acquisition::Secondary(_) => panic!("a free data root must make the launch primary"),
    }
}

fn secondary(root: &TempDir) -> RunningInstance {
    match acquire(root.path()).expect("acquire a held data root") {
        Acquisition::Secondary(running) => running,
        Acquisition::Primary(_) => panic!("a held data root must make the launch secondary"),
    }
}

#[tokio::test]
async fn second_launch_hands_its_arguments_to_the_primary() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut guard = primary(&root);
    let running = secondary(&root);
    let sent = HandOff {
        arguments: vec![
            "open".to_owned(),
            "a path with \"quotes\" and spaces".to_owned(),
        ],
        working_directory: Some(root.path().to_path_buf()),
    };

    let (received, delivered) =
        tokio::join!(timeout(ROUND, guard.accept()), running.hand_off(&sent));

    delivered.expect("the primary acknowledges the hand-off");
    let received = received.expect("the primary accepts in time");
    assert_eq!(received.expect("the primary receives the hand-off"), sent);
}

#[tokio::test]
async fn primary_serves_one_launch_after_another() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut guard = primary(&root);

    for round in 0..3 {
        let running = secondary(&root);
        let sent = HandOff {
            arguments: vec![format!("round-{round}")],
            working_directory: None,
        };
        let (received, delivered) =
            tokio::join!(timeout(ROUND, guard.accept()), running.hand_off(&sent));
        delivered.expect("each launch is acknowledged");
        let received = received.expect("the primary accepts each launch in time");
        assert_eq!(received.expect("each hand-off arrives"), sent);
    }
}

#[tokio::test]
async fn data_roots_do_not_share_a_lock() {
    let first = tempfile::tempdir().expect("tempdir");
    let second = tempfile::tempdir().expect("tempdir");

    let _held = primary(&first);
    let _also_held = primary(&second);

    assert_eq!(
        secondary(&first).data_root(),
        first.path().canonicalize().expect("canonical data root")
    );
}

#[tokio::test]
async fn dropping_the_guard_frees_the_data_root() {
    let root = tempfile::tempdir().expect("tempdir");
    let guard = primary(&root);
    drop(guard);

    let _again = primary(&root);
}

#[tokio::test]
async fn hand_off_without_a_listening_primary_is_unreachable() {
    let root = tempfile::tempdir().expect("tempdir");
    let guard = primary(&root);
    let running = secondary(&root);
    drop(guard);

    let outcome = running.hand_off(&HandOff::default()).await;

    assert!(
        matches!(outcome, Err(InstanceError::Unreachable { .. })),
        "a launch must learn that nothing answered, got {outcome:?}"
    );
}

#[test]
fn acquire_outside_a_runtime_is_refused_without_touching_the_data_root() {
    let root = tempfile::tempdir().expect("tempdir");
    let data_root = root.path().join("never-created");

    let outcome = acquire(&data_root);

    assert!(matches!(outcome, Err(InstanceError::NoRuntime { .. })));
    assert!(!data_root.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn data_root_and_socket_are_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = tempfile::tempdir().expect("tempdir");
    let data_root = root.path().join("root");
    let _guard = match acquire(&data_root).expect("acquire") {
        Acquisition::Primary(guard) => guard,
        Acquisition::Secondary(_) => panic!("a new data root must make the launch primary"),
    };

    let mode = |name: &str| {
        std::fs::metadata(data_root.join(name))
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode(""), 0o700);
    assert_eq!(mode("instance.sock"), 0o600);
}

#[cfg(unix)]
#[tokio::test]
async fn data_root_too_deep_for_a_socket_address_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let data_root = root.path().join("d".repeat(120));

    let outcome = acquire(&data_root);

    assert!(
        matches!(outcome, Err(InstanceError::EndpointTooLong { .. })),
        "got {outcome:?}"
    );
}

#[tokio::test]
async fn second_launch_in_another_process_hands_off_and_exits() {
    let root = tempfile::tempdir().expect("tempdir");
    let mut guard = primary(&root);

    let child = spawn_second_launch(&root, EXPECT_DELIVERED);
    let received = timeout(ROUND, guard.accept())
        .await
        .expect("the primary accepts in time")
        .expect("the primary receives the hand-off");

    assert_eq!(received.arguments, vec![HELPER_ARGUMENT.to_owned()]);
    assert!(
        exit_of(child).await.success(),
        "the second launch reports its hand-off"
    );
}

#[tokio::test]
async fn second_launch_exits_when_the_primary_never_accepts() {
    let root = tempfile::tempdir().expect("tempdir");
    let _guard = primary(&root);

    let child = spawn_second_launch(&root, EXPECT_REFUSED);

    assert!(
        exit_of(child).await.success(),
        "the second launch gives up and exits"
    );
}
