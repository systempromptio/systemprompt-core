use std::time::Duration;

use systemprompt_device::platform::instance::{
    Acquisition, HandOff, InstanceError, InstanceGuard, RunningInstance, acquire,
};
use tempfile::TempDir;
use tokio::time::timeout;

// Why: the primary keeps listening after a hand-off it rejected, so an
// unbounded accept would hide the launch's own error behind a hung test.
const ROUND: Duration = Duration::from_secs(20);

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
