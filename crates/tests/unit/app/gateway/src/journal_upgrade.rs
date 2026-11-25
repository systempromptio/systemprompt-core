use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use systemprompt_gateway::{GatewayAtRest, GatewayAtRestError};
use systemprompt_test_fixtures::master_key_of;

const MASTER: [u8; 32] = [0x5a; 32];

fn pre_088_segment(dir: &std::path::Path) -> std::path::PathBuf {
    let root = dir.join("gateway-journal");
    std::fs::create_dir_all(&root).expect("journal dir");
    let nonce: [u8; 12] = rand::random();
    let sealed = ChaCha20Poly1305::new(&MASTER.into())
        .encrypt(&Nonce::from(nonce), br#"{"Remove":"req-1"}"#.as_slice())
        .expect("raw seal");
    let mut body = nonce.to_vec();
    body.extend_from_slice(&sealed);
    let mut frame = u32::try_from(body.len())
        .expect("len")
        .to_le_bytes()
        .to_vec();
    frame.extend_from_slice(&body);
    let path = root.join("oldowner-0000000001.seg");
    std::fs::write(&path, frame).expect("plant the segment");
    path
}

fn at_rest(dir: &std::path::Path) -> GatewayAtRest {
    GatewayAtRest::new(Some(dir.to_path_buf()), Some(master_key_of(MASTER)))
}

#[test]
fn a_clean_upgrade_opens_the_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let at_rest = at_rest(dir.path());
    let report = at_rest.reseal_journal(false).expect("dry run");
    assert_eq!(report.pre_088, 0);
    at_rest
        .journal()
        .expect("no pre-0.88 segment: the journal opens");
}

#[test]
fn a_pending_pre_088_segment_refuses_the_journal_until_it_is_resealed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let segment = pre_088_segment(dir.path());
    let before = std::fs::read(&segment).expect("read");
    let at_rest = at_rest(dir.path());
    assert!(
        matches!(
            at_rest.journal(),
            Err(GatewayAtRestError::PreUpgradeJournal { segments: 1 })
        ),
        "a pre-0.88 segment is never opened as torn"
    );
    let dry = at_rest.reseal_journal(false).expect("dry run");
    assert_eq!((dry.pre_088, dry.resealed), (1, 0));
    assert_eq!(
        std::fs::read(&segment).expect("read"),
        before,
        "a dry run writes nothing"
    );
    let applied = at_rest.reseal_journal(true).expect("apply");
    assert_eq!((applied.pre_088, applied.resealed), (1, 1));
    let again = at_rest.reseal_journal(false).expect("again");
    assert_eq!(
        (again.pre_088, again.current),
        (0, 1),
        "the segment is under the journal key"
    );
    at_rest
        .journal()
        .expect("the journal opens once the segment is re-sealed");
}
