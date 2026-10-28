use systemprompt_engine_loro::{LoroEngine, WIRE_VERSION};
use systemprompt_files::documents::engine::{DocumentEngine, EngineError, ModelView};

#[test]
fn wire_version_is_the_pinned_loro_crate() {
    assert_eq!(LoroEngine.wire_version(), WIRE_VERSION);
    assert_eq!(WIRE_VERSION, "1.17.1");
}

#[test]
fn seeded_replicas_converge_on_one_update() {
    let model = ModelView::Body("<p>One</p>".to_owned());
    let mut left = LoroEngine.seed(&model).expect("seed");
    let mut right = LoroEngine.seed(&model).expect("seed");
    let target = ModelView::Body("<p>One and two</p>".to_owned());
    let update = left
        .update_to_model(&target)
        .expect("diff")
        .expect("changed");
    left.apply_update(&update).expect("left");
    right.apply_update(&update).expect("right");
    assert_eq!(left.export_model(), target);
    assert_eq!(right.export_model(), target);
    assert!(left.update_to_model(&target).expect("diff").is_none());
}

#[test]
fn a_snapshot_reopens_with_its_body_and_version() {
    let doc = LoroEngine
        .seed(&ModelView::Body("hello".to_owned()))
        .expect("seed");
    let reopened = LoroEngine
        .open(&doc.snapshot().expect("snapshot"))
        .expect("open");
    assert_eq!(reopened.export_model(), doc.export_model());
    assert!(
        reopened
            .missing_since(&doc.version())
            .expect("missing")
            .is_none()
    );
}

#[test]
fn garbage_is_refused_as_an_update_and_as_a_version() {
    assert!(matches!(
        LoroEngine.check_update(b"not loro"),
        Err(EngineError::InvalidUpdate(_))
    ));
    let doc = LoroEngine
        .seed(&ModelView::Body(String::new()))
        .expect("seed");
    assert!(matches!(
        doc.missing_since(b"\xff\xff"),
        Err(EngineError::InvalidVersion(_))
    ));
}
