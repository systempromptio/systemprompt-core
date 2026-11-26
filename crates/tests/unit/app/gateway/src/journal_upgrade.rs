use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use systemprompt_gateway::audit::journal::GatewayJournal;
use systemprompt_gateway::{GatewayAtRest, GatewayAtRestError, GatewayAuditError};
use systemprompt_test_fixtures::master_key_of;

const MASTER: [u8; 32] = [0x5a; 32];

fn framed(body: &[u8]) -> Vec<u8> {
    let mut frame = u32::try_from(body.len())
        .expect("len")
        .to_le_bytes()
        .to_vec();
    frame.extend_from_slice(body);
    frame
}

fn pre_088_frame() -> Vec<u8> {
    let nonce: [u8; 12] = rand::random();
    let sealed = ChaCha20Poly1305::new(&MASTER.into())
        .encrypt(&Nonce::from(nonce), br#"{"Remove":"req-1"}"#.as_slice())
        .expect("raw seal");
    let mut body = nonce.to_vec();
    body.extend_from_slice(&sealed);
    framed(&body)
}

fn damaged_frame() -> Vec<u8> {
    framed(&[0x42; 40])
}

fn plant(dir: &std::path::Path, frames: &[Vec<u8>]) -> std::path::PathBuf {
    let root = dir.join("gateway-journal");
    std::fs::create_dir_all(&root).expect("journal dir");
    let path = root.join("oldowner-0000000001.seg");
    std::fs::write(&path, frames.concat()).expect("plant the segment");
    path
}

fn pre_088_segment(dir: &std::path::Path) -> std::path::PathBuf {
    plant(dir, &[pre_088_frame()])
}

fn refused_as_pre_088(at_rest: &GatewayAtRest) -> bool {
    matches!(
        at_rest.journal(),
        Err(GatewayAtRestError::Journal(
            GatewayAuditError::PreUpgradeJournal { segments: 1 }
        ))
    )
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
        refused_as_pre_088(&at_rest),
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

#[test]
fn a_damaged_first_frame_does_not_hide_the_pre_088_frames_after_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    let segment = plant(
        dir.path(),
        &[damaged_frame(), pre_088_frame(), pre_088_frame()],
    );
    let at_rest = at_rest(dir.path());
    let dry = at_rest.reseal_journal(false).expect("dry run");
    assert_eq!(
        (dry.pre_088, dry.undecodable),
        (1, 0),
        "a segment with any pre-0.88 frame is judged pre-0.88, not undecodable"
    );
    assert!(
        refused_as_pre_088(&at_rest),
        "the journal never opens, so recovery never deletes the segment"
    );
    assert!(segment.exists());
    let applied = at_rest.reseal_journal(true).expect("apply");
    assert_eq!(applied.resealed, 1);
    let again = at_rest.reseal_journal(false).expect("again");
    assert_eq!((again.pre_088, again.current), (0, 1));
    at_rest
        .journal()
        .expect("the journal opens once the intact frames are re-sealed");
}

#[test]
fn a_reseal_keeps_the_frames_no_key_opens_and_the_torn_tail() {
    let dir = tempfile::tempdir().expect("tempdir");
    let damaged = damaged_frame();
    let torn = b"\x10\x00\x00\x00half".to_vec();
    let segment = plant(
        dir.path(),
        &[
            pre_088_frame(),
            damaged.clone(),
            pre_088_frame(),
            torn.clone(),
        ],
    );
    let at_rest = at_rest(dir.path());
    let dry = at_rest.reseal_journal(false).expect("dry run");
    assert_eq!(
        (dry.pre_088, dry.frames_kept_unopened),
        (1, 2),
        "the damaged frame and the torn tail are counted, not dropped"
    );
    let applied = at_rest.reseal_journal(true).expect("apply");
    assert_eq!((applied.resealed, applied.frames_kept_unopened), (1, 2));
    let bytes = std::fs::read(&segment).expect("read");
    let mut kept = damaged;
    kept.extend_from_slice(&torn);
    assert!(
        bytes.ends_with(&kept),
        "the unopened frame and the torn tail are kept verbatim after the resealed frames"
    );
    let again = at_rest.reseal_journal(false).expect("again");
    assert_eq!((again.pre_088, again.current), (0, 1));
}

#[test]
fn a_segment_no_key_opens_is_undecodable_and_does_not_block_the_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    plant(dir.path(), &[damaged_frame(), damaged_frame()]);
    let at_rest = at_rest(dir.path());
    let dry = at_rest.reseal_journal(false).expect("dry run");
    assert_eq!((dry.pre_088, dry.undecodable), (0, 1));
    at_rest
        .journal()
        .expect("a segment no key opens holds nothing to settle");
}

#[test]
fn a_staged_file_left_by_a_dead_run_is_replaced_from_the_untouched_segment() {
    let dir = tempfile::tempdir().expect("tempdir");
    let segment = pre_088_segment(dir.path());
    let staged = segment.with_extension("reseal");
    std::fs::write(&staged, b"half a frame from a run that died").expect("plant the staged file");
    let at_rest = at_rest(dir.path());
    let applied = at_rest
        .reseal_journal(true)
        .expect("a stale staged file never blocks the re-seal");
    assert_eq!(applied.resealed, 1);
    assert!(!staged.exists(), "the staged file was renamed in");
    let again = at_rest.reseal_journal(false).expect("again");
    assert_eq!((again.current, again.undecodable), (1, 0));
    at_rest.journal().expect("the journal opens");
}

#[test]
fn opening_the_journal_directly_refuses_a_pre_088_segment() {
    let dir = tempfile::tempdir().expect("tempdir");
    pre_088_segment(dir.path());
    assert!(
        matches!(
            GatewayJournal::open(dir.path(), &master_key_of(MASTER)),
            Err(GatewayAuditError::PreUpgradeJournal { segments: 1 })
        ),
        "no entry point opens a journal whose recovery could discard pre-0.88 frames"
    );
}
