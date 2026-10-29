use systemprompt_files::documents::codecs::docx::{import_rich, materialise_rich};
use systemprompt_files::documents::codecs::odf::{import_odt_rich, materialise_odt_rich};
use systemprompt_files::documents::model::{Block, BlockKind, Mark, MarkKind};

use super::docx_fixture::{DOCUMENT_XML, entries, package_with, part};
use super::odf_fixture::{assert_only_content_changed, package};

const ODT: &str = "application/vnd.oasis.opendocument.text";

fn odt_body() -> String {
    concat!(
        "<office:text><text:sequence-decls/>",
        "<text:h text:outline-level=\"2\">Plan</text:h>",
        "<text:p text:style-name=\"P1\">Keep <text:span text:style-name=\"T1\">this</text:span>.</text:p>",
        "<table:table table:name=\"T\"><table:table-row><table:table-cell><text:p>x</text:p></table:table-cell></table:table-row></table:table>",
        "</office:text>"
    )
    .to_owned()
}

#[test]
fn a_docx_body_imports_headings_lists_marks_tables_and_raw_objects() {
    let model = import_rich(&package_with(DOCUMENT_XML)).expect("import");
    let kinds: Vec<_> = model.blocks.iter().map(|b| &b.kind).collect();
    assert_eq!(kinds[0], &BlockKind::Heading { level: 1 });
    assert_eq!(model.blocks[1].text, "Revenue grew strongly.");
    assert_eq!(
        model.blocks[1].marks,
        vec![Mark {
            start: 0,
            end: 12,
            kind: MarkKind::Bold
        }]
    );
    assert_eq!(
        kinds[2],
        &BlockKind::ListItem {
            level: 0,
            ordered: false,
            numbering: None,
            checked: None,
        }
    );
    assert!(
        matches!(kinds[3], BlockKind::Raw { .. }),
        "the drawing paragraph is kept verbatim"
    );
    assert!(
        matches!(kinds[4], BlockKind::Table { .. }),
        "the table imports as an editable table"
    );
}

#[test]
fn an_unchanged_docx_model_materialises_to_the_same_document() {
    let base = package_with(DOCUMENT_XML);
    let model = import_rich(&base).expect("import");
    let out = materialise_rich(&base, &model).expect("materialise");
    assert_eq!(
        part(&out, "word/document.xml"),
        part(&base, "word/document.xml")
    );
}

#[test]
fn a_docx_edit_rewrites_only_its_paragraph_and_leaves_other_parts_byte_identical() {
    let base = package_with(DOCUMENT_XML);
    let mut model = import_rich(&base).expect("import");
    model.blocks[0].text = "Annual Report".to_owned();
    model
        .blocks
        .push(Block::new("n.1", BlockKind::Paragraph, "Added"));
    let out = materialise_rich(&base, &model).expect("materialise");
    let xml = part(&out, "word/document.xml");
    assert!(xml.contains("Annual Report"));
    assert!(
        xml.contains("<w:t xml:space=\"preserve\">Added</w:t>") || xml.contains(">Added</w:t>")
    );
    assert!(
        xml.contains("<w:bookmarkStart w:id=\"0\" w:name=\"intro\"/>"),
        "untouched paragraphs keep their markup"
    );
    assert!(xml.contains("<w:sectPr>"), "the section properties survive");
    for (old, new) in entries(&base).iter().zip(entries(&out).iter()) {
        if old.0 != "word/document.xml" {
            assert_eq!(old, new, "{} is untouched", old.0);
        }
    }
    let again = import_rich(&out).expect("reimport");
    assert_eq!(again.blocks[0].text, "Annual Report");
    assert_eq!(again.blocks.last().map(|b| b.text.as_str()), Some("Added"));
}

#[test]
fn an_odt_body_round_trips_and_an_edit_touches_only_content_xml() {
    let base = package(ODT, &odt_body());
    let model = import_odt_rich(&base).expect("import");
    assert!(
        matches!(model.blocks[0].kind, BlockKind::Raw { .. }),
        "declarations are kept"
    );
    assert_eq!(model.blocks[1].kind, BlockKind::Heading { level: 2 });
    assert_eq!(model.blocks[2].text, "Keep this.");
    assert!(matches!(model.blocks[3].kind, BlockKind::Raw { .. }));
    let same = materialise_odt_rich(&base, &model).expect("materialise");
    assert_eq!(entries(&same), entries(&base));
    let mut edited = model;
    edited.blocks[1].text = "Plan B".to_owned();
    let out = materialise_odt_rich(&base, &edited).expect("materialise");
    assert_only_content_changed(&base, &out);
    let again = import_odt_rich(&out).expect("reimport");
    assert_eq!(again.blocks[1].text, "Plan B");
    assert_eq!(again.blocks[2].text, "Keep this.");
}
