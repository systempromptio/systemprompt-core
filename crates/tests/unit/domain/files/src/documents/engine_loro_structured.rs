use std::collections::BTreeMap;

use loro::{LoroDoc, LoroMap, LoroMovableList, LoroText};
use systemprompt_files::documents::engine::loro::deck::{
    DeckModel, DeckShape, DeckSlide, ShapeKind,
};
use systemprompt_files::documents::engine::loro::sheet::{Sheet, SheetCell, SheetModel};
use systemprompt_files::documents::engine::loro::table::{TABLE_KIND, canonical};
use systemprompt_files::documents::engine::loro::{LoroEngine, LoroEngineDoc, rich};
use systemprompt_files::documents::engine::{
    DocumentEngine, EngineDoc, EngineError, ImportScope, ModelView,
};
use systemprompt_files::documents::model::layout::{CellProps, TableProps};
use systemprompt_files::documents::model::{Block, BlockKind, RichText};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{NodeKind, ProjectionNode};

fn input(v: &str) -> SheetCell {
    SheetCell {
        v: v.to_owned(),
        ..SheetCell::default()
    }
}

fn workbook(cells: &[(u32, u32, &str)]) -> ModelView {
    ModelView::Sheet(SheetModel {
        sheets: vec![Sheet::from_grid(
            "Data",
            cells.iter().map(|(c, r, v)| (*c, *r, input(v))),
            &[],
        )],
    })
}

fn deck(title: &str) -> ModelView {
    ModelView::Deck(DeckModel {
        slides: vec![DeckSlide {
            id: "s1".to_owned(),
            layout: None,
            shapes: vec![DeckShape {
                id: "s1.t".to_owned(),
                kind: ShapeKind::Title,
                frame: None,
                paragraphs: vec![Block::new("s1.p", BlockKind::Paragraph, title)],
                src: None,
            }],
        }],
    })
}

fn update_toward(doc: &dyn EngineDoc, model: &ModelView) -> Vec<u8> {
    doc.fork()
        .update_to_model_as(model, 77)
        .expect("diff")
        .expect("changed")
}

#[test]
fn engine_loro_scoped_sheet_import_reports_full_before_and_after() {
    let engine = LoroEngine;
    let mut doc = engine.seed(&workbook(&[(1, 1, "2")])).expect("seed");
    let target = workbook(&[(1, 1, "2"), (2, 3, "=A1*2")]);
    let update = update_toward(doc.as_ref(), &target);
    let scoped = doc.apply_update_scoped(&update, true).expect("import");
    let ImportScope::Full { before, after } = scoped.scope else {
        panic!("a workbook import is never block-scoped");
    };
    assert_eq!(before.kind(), "sheet");
    assert_eq!(after, doc.export_model().expect("export"));
    assert_eq!(scoped.import.peers, vec![77]);
    assert!(doc.update_to_model(&after).expect("diff").is_none());
}

#[test]
fn engine_loro_scoped_sheet_import_refuses_a_broken_formula() {
    let engine = LoroEngine;
    let mut doc = engine.seed(&workbook(&[(1, 1, "2")])).expect("seed");
    let update = update_toward(doc.as_ref(), &workbook(&[(1, 1, "2"), (1, 2, "=SUM(")]));
    let refused = doc.apply_update_scoped(&update, true);
    assert!(matches!(refused, Err(EngineError::Model(_))));
}

#[test]
fn engine_loro_scoped_deck_import_validates_the_whole_deck() {
    let engine = LoroEngine;
    let mut doc = engine.seed(&deck("Old")).expect("seed");
    let target = deck("New title");
    let update = update_toward(doc.as_ref(), &target);
    let scoped = doc.apply_update_scoped(&update, true).expect("import");
    let ImportScope::Full { before, after } = scoped.scope else {
        panic!("a deck import is never block-scoped");
    };
    assert_eq!(before, deck("Old"));
    assert_eq!(after, target);
}

fn para(id: &str, text: &str) -> Block {
    Block::new(id, BlockKind::Paragraph, text)
}

fn cell(id: &str, text: &str) -> RichText {
    RichText::of(vec![para(id, text)])
}

fn table_doc() -> RichText {
    RichText::of(vec![
        para("p0", "intro"),
        Block::new(
            "t0",
            BlockKind::Table {
                rows: vec![vec![cell("c0", "a"), cell("c1", "b")]],
                props: TableProps::default(),
            },
            "",
        ),
    ])
}

fn node(id: &str, kind: NodeKind, children: Vec<ProjectionNode>) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(id),
        kind,
        text: None,
        hash8: String::new(),
        children,
        attrs: BTreeMap::new(),
    }
}

fn projection(cells: usize) -> Vec<ProjectionNode> {
    let cell_nodes = (0..cells)
        .map(|c| {
            node(
                &format!("cell{c}"),
                NodeKind::Cell,
                vec![node(&format!("cp{c}"), NodeKind::Paragraph, Vec::new())],
            )
        })
        .collect();
    vec![node(
        "body",
        NodeKind::Body,
        vec![
            node("block[1]", NodeKind::Paragraph, Vec::new()),
            node(
                "table[1]",
                NodeKind::Table,
                vec![node("row[1]", NodeKind::Row, cell_nodes)],
            ),
        ],
    )]
}

fn ids_of(model: &ModelView) -> Vec<String> {
    let ModelView::RichText(rich) = model else {
        panic!("rich");
    };
    let mut ids: Vec<String> = rich.blocks.iter().map(|b| b.id.clone()).collect();
    if let BlockKind::Table { rows, .. } = &rich.blocks[1].kind {
        ids.extend(
            rows.iter()
                .flatten()
                .flat_map(|c| c.blocks.iter().map(|b| b.id.clone())),
        );
    }
    ids
}

#[test]
fn engine_loro_adopt_writes_projection_ids_when_the_tree_aligns() {
    let mut doc = LoroEngine
        .seed(&ModelView::RichText(table_doc()))
        .expect("seed");
    assert!(doc.adopt_node_ids(&projection(2)).expect("adopt"));
    let model = doc.export_model().expect("export");
    assert_eq!(ids_of(&model), ["block[1]", "table[1]", "cp0", "cp1"]);
    assert!(doc.adopt_node_ids(&projection(2)).expect("again"));
    assert_eq!(doc.export_model().expect("export"), model);
}

#[test]
fn engine_loro_adopt_leaves_a_misaligned_tree_untouched() {
    let mut doc = LoroEngine
        .seed(&ModelView::RichText(table_doc()))
        .expect("seed");
    let version = doc.version();
    assert!(!doc.adopt_node_ids(&projection(3)).expect("cells"));
    let swapped = vec![
        node("x", NodeKind::Table, Vec::new()),
        node("y", NodeKind::Paragraph, Vec::new()),
    ];
    assert!(!doc.adopt_node_ids(&swapped).expect("kinds"));
    let row_kind = vec![
        node("x", NodeKind::Paragraph, Vec::new()),
        node(
            "t",
            NodeKind::Table,
            vec![node("r", NodeKind::Cell, Vec::new())],
        ),
    ];
    assert!(!doc.adopt_node_ids(&row_kind).expect("row"));
    assert!(!doc.adopt_node_ids(&[]).expect("empty"));
    assert_eq!(doc.version(), version);
    assert_eq!(
        ids_of(&doc.export_model().expect("export")),
        ["p0", "t0", "c0", "c1"]
    );
}

fn legacy_table_doc() -> LoroDoc {
    let doc = LoroDoc::new();
    rich::configure(&doc);
    let table = rich::blocks(&doc)
        .insert_container(0, LoroMap::new())
        .expect("block");
    table.insert("id", "t0").expect("id");
    table.insert("kind", TABLE_KIND).expect("kind");
    let rows = table
        .insert_container("rows", LoroMovableList::new())
        .expect("rows");
    let row = rows.insert_container(0, LoroMap::new()).expect("row");
    row.insert("id", "r0").expect("row id");
    let cells = row
        .insert_container("cells", LoroMovableList::new())
        .expect("cells");
    let cell = cells.insert_container(0, LoroMap::new()).expect("cell");
    cell.insert("id", "k0").expect("cell id");
    let blocks = cell
        .insert_container("blocks", LoroMovableList::new())
        .expect("blocks");
    let para = blocks.insert_container(0, LoroMap::new()).expect("para");
    para.insert("id", "c0").expect("para id");
    para.insert("kind", r#"{"type":"paragraph"}"#)
        .expect("para kind");
    para.insert_container("text", LoroText::new())
        .expect("text")
        .insert(0, "legacy")
        .expect("insert");
    doc.commit();
    doc
}

#[test]
fn engine_loro_a_legacy_table_upgrades_when_its_own_model_is_written_back() {
    let doc = LoroEngineDoc(legacy_table_doc());
    let model = doc.export_model().expect("export");
    let ModelView::RichText(rich) = &model else {
        panic!("rich");
    };
    let BlockKind::Table { rows, .. } = &rich.blocks[0].kind else {
        panic!("table");
    };
    assert_eq!(rows[0][0].blocks[0].text, "legacy");
    let update = doc.update_to_model(&model).expect("diff").expect("upgrade");
    let mut upgraded = doc.fork();
    upgraded.apply_update(&update).expect("apply");
    assert_eq!(upgraded.export_model().expect("export"), model);
    assert!(upgraded.update_to_model(&model).expect("diff").is_none());
}

#[test]
fn engine_loro_canonical_drops_default_cell_styling_recursively() {
    let inner = Block::new(
        "inner",
        BlockKind::Table {
            rows: vec![vec![cell("i0", "x")]],
            props: TableProps {
                cells: vec![vec![CellProps {
                    colspan: 1,
                    rowspan: 1,
                    ..CellProps::default()
                }]],
                ..TableProps::default()
            },
        },
        "",
    );
    let mut model = RichText::of(vec![Block::new(
        "outer",
        BlockKind::Table {
            rows: vec![vec![RichText::of(vec![inner])]],
            props: TableProps {
                cells: vec![vec![CellProps::default()], vec![CellProps::default()]],
                ..TableProps::default()
            },
        },
        "",
    )]);
    canonical(&mut model);
    let BlockKind::Table { rows, props } = &model.blocks[0].kind else {
        panic!("table");
    };
    assert!(props.cells.is_empty());
    let BlockKind::Table { props: inner, .. } = &rows[0][0].blocks[0].kind else {
        panic!("inner table");
    };
    assert!(inner.cells.is_empty());
}
