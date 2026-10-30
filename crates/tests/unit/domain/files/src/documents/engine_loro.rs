use systemprompt_files::documents::engine::loro::{
    LoroEngine, MAX_UPDATE_BYTES, UpdateShapeError, WIRE_VERSION, check_update_shape,
};
use systemprompt_files::documents::engine::{DocumentEngine, EngineError, ModelView};
use systemprompt_files::documents::model::{Block, BlockKind, Mark, MarkKind, RichText};

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
    assert_eq!(left.export_model().expect("export"), target);
    assert_eq!(right.export_model().expect("export"), target);
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
    assert_eq!(
        reopened.export_model().expect("export"),
        doc.export_model().expect("export")
    );
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
        LoroEngine.check_update(b"not loro", None),
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

fn rich(blocks: Vec<Block>) -> ModelView {
    ModelView::RichText(RichText::of(blocks))
}

#[test]
fn rich_text_replicas_converge_on_concurrent_edits() {
    let seed = rich(vec![
        Block::new("b1", BlockKind::Heading { level: 1 }, "Title"),
        Block::new("b2", BlockKind::Paragraph, "Body"),
    ]);
    let mut left = LoroEngine.seed(&seed).expect("seed");
    let mut right = LoroEngine.seed(&seed).expect("seed");
    let mut bold = Block::new("b2", BlockKind::Paragraph, "Body text");
    bold.marks.push(Mark {
        start: 0,
        end: 4,
        kind: MarkKind::Bold,
    });
    let from_left = left
        .update_to_model(&rich(vec![
            Block::new("b1", BlockKind::Heading { level: 1 }, "Title"),
            bold,
        ]))
        .expect("diff")
        .expect("changed");
    let from_right = right
        .update_to_model(&rich(vec![
            Block::new("b1", BlockKind::Heading { level: 2 }, "Title"),
            Block::new("b2", BlockKind::Paragraph, "Body"),
            Block::new("r.1", BlockKind::Paragraph, "Added"),
        ]))
        .expect("diff")
        .expect("changed");
    for doc in [&mut left, &mut right] {
        doc.apply_update(&from_left).expect("left update");
        doc.apply_update(&from_right).expect("right update");
    }
    assert_eq!(
        left.export_model().expect("export"),
        right.export_model().expect("export")
    );
    let ModelView::RichText(merged) = left.export_model().expect("export") else {
        panic!("rich text");
    };
    assert_eq!(merged.blocks.len(), 3);
    assert_eq!(merged.blocks[0].kind, BlockKind::Heading { level: 2 });
    assert_eq!(merged.blocks[1].text, "Body text");
    assert_eq!(
        merged.blocks[1].marks,
        vec![Mark {
            start: 0,
            end: 4,
            kind: MarkKind::Bold
        }]
    );
}

#[test]
fn a_fork_takes_an_update_without_changing_the_document() {
    let doc = LoroEngine
        .seed(&rich(vec![Block::new("b1", BlockKind::Paragraph, "a")]))
        .expect("seed");
    let update = doc
        .update_to_model(&rich(vec![Block::new("b1", BlockKind::Paragraph, "ab")]))
        .expect("diff")
        .expect("changed");
    let mut fork = doc.fork();
    fork.apply_update(&update).expect("apply");
    assert_ne!(
        fork.export_model().expect("export"),
        doc.export_model().expect("export")
    );
    assert_eq!(
        doc.export_model().expect("export"),
        rich(vec![Block::new("b1", BlockKind::Paragraph, "a")])
    );
}

fn one_update_of(peer: u64) -> Vec<u8> {
    let doc = loro::LoroDoc::new();
    doc.set_peer_id(peer).expect("peer");
    doc.get_text("body").insert(0, "hello").expect("insert");
    doc.commit();
    doc.export(loro::ExportMode::all_updates()).expect("export")
}

#[test]
fn a_snapshot_posted_as_an_update_is_refused_from_its_header() {
    let doc = LoroEngine
        .seed(&ModelView::Body("<p>x</p>".to_owned()))
        .expect("seed");
    let snapshot = doc.snapshot().expect("snapshot");
    assert!(matches!(
        check_update_shape(&snapshot, None),
        Err(UpdateShapeError::NotAnUpdate { .. })
    ));
    assert!(matches!(
        LoroEngine.check_update(&snapshot, None),
        Err(EngineError::InvalidUpdate(_))
    ));
}

#[test]
fn an_update_of_an_unassigned_peer_is_refused_before_import() {
    let update = one_update_of(42);
    assert!(LoroEngine.check_update(&update, Some(&[42])).is_ok());
    assert!(LoroEngine.check_update(&update, None).is_ok());
    assert!(matches!(
        check_update_shape(&update, Some(&[7])),
        Err(UpdateShapeError::PeerNotAssigned { peer: 42 })
    ));
}

#[test]
fn an_oversized_blob_is_refused_without_decoding() {
    let blob = vec![0_u8; MAX_UPDATE_BYTES + 1];
    assert!(matches!(
        check_update_shape(&blob, None),
        Err(UpdateShapeError::TooLarge { .. })
    ));
}
