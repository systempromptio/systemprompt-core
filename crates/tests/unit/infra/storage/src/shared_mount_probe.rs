use systemprompt_identifiers::InstanceId;
use systemprompt_storage::probe_shared_mount;
use tempfile::TempDir;

#[tokio::test]
async fn first_instance_sees_no_siblings_and_reads_its_marker_back() {
    let dir = TempDir::new().expect("tempdir");
    let report = probe_shared_mount(dir.path(), &InstanceId::new("node-a"))
        .await
        .expect("probe");

    assert!(report.write_read_ok);
    assert!(report.instances.is_empty());
    assert!(!report.has_siblings());
    let marker = dir.path().join(".systemprompt/instances/node-a");
    let body = std::fs::read_to_string(&marker).expect("marker written");
    assert!(
        chrono_like(&body),
        "marker body must be an RFC3339 timestamp, got {body}"
    );
}

#[tokio::test]
async fn second_instance_sees_the_first_as_a_sibling() {
    let dir = TempDir::new().expect("tempdir");
    probe_shared_mount(dir.path(), &InstanceId::new("node-a"))
        .await
        .expect("first probe");
    let report = probe_shared_mount(dir.path(), &InstanceId::new("node-b"))
        .await
        .expect("second probe");

    assert!(report.write_read_ok);
    assert_eq!(report.instances, vec!["node-a".to_owned()]);
    assert!(report.has_siblings());
}

#[tokio::test]
async fn probe_creates_a_missing_root() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().join("mount/storage");
    let report = probe_shared_mount(&root, &InstanceId::new("node-a"))
        .await
        .expect("probe");
    assert!(report.write_read_ok);
    assert!(root.join(".systemprompt/instances/node-a").is_file());
}

#[tokio::test]
async fn unwritable_root_fails_the_probe() {
    let dir = TempDir::new().expect("tempdir");
    let blocker = dir.path().join("not-a-dir");
    std::fs::write(&blocker, b"file").expect("blocker");
    probe_shared_mount(&blocker, &InstanceId::new("node-a"))
        .await
        .expect_err("a root that is a regular file cannot hold markers");
}

fn chrono_like(body: &str) -> bool {
    body.len() >= 20 && body.as_bytes()[4] == b'-' && body.contains('T')
}
