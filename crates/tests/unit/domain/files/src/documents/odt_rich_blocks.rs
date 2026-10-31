use systemprompt_files::documents::codecs::odf::{import_odt_rich, materialise_odt_rich};
use systemprompt_files::documents::model::{Block, BlockKind, RichText};

use super::docx_fixture::part;
use super::odf_fixture::package;

const ODT: &str = "application/vnd.oasis.opendocument.text";

fn odt(body: &str) -> Vec<u8> {
    package(ODT, &format!("<office:text>{body}</office:text>"))
}

fn list_item() -> BlockKind {
    BlockKind::ListItem {
        level: 0,
        ordered: false,
        numbering: None,
        checked: None,
    }
}

#[test]
fn heading_levels_clamp_and_objects_stay_raw() {
    let base = odt(concat!(
        "<text:h text:outline-level=\"9\">Deep</text:h>",
        "<text:h>Plain</text:h>",
        "<text:h text:outline-level=\"x\">Odd</text:h>",
        "<text:p>See <draw:frame><draw:image xlink:href=\"Pictures/x.png\"/></draw:frame></text:p>",
        "<text:p><text:span><text:note><text:note-body/></text:note></text:span></text:p>",
        "<text:section><text:p>inside</text:p></text:section>",
    ));
    let model = import_odt_rich(&base).expect("import");
    assert_eq!(model.blocks[0].kind, BlockKind::Heading { level: 6 });
    assert_eq!(model.blocks[0].text, "Deep");
    assert_eq!(model.blocks[1].kind, BlockKind::Heading { level: 1 });
    assert_eq!(model.blocks[2].kind, BlockKind::Heading { level: 1 });
    for at in 3..6 {
        assert!(
            matches!(&model.blocks[at].kind, BlockKind::Raw { source } if !source.is_empty()),
            "{:?}",
            model.blocks[at].kind
        );
    }
    assert!(
        matches!(&model.blocks[3].kind, BlockKind::Raw { source } if source.contains("draw:frame"))
    );
}

#[test]
fn new_blocks_are_written_as_odf_elements_and_read_back() {
    let base = odt("<text:p>first</text:p>");
    let mut model = import_odt_rich(&base).expect("import");
    model.blocks.push(Block::new(
        "h",
        BlockKind::Heading { level: 3 },
        "Title & more",
    ));
    model
        .blocks
        .push(Block::new("l", list_item(), "item <one>"));
    model.blocks.push(Block::new(
        "r",
        BlockKind::Raw {
            source: "<text:p>raw kept</text:p>".to_owned(),
        },
        "",
    ));
    model
        .blocks
        .push(Block::new("q", BlockKind::Quote, "quoted"));
    let out = materialise_odt_rich(&base, &model).expect("materialise");
    let content = part(&out, "content.xml");
    assert!(content.contains("<text:h text:outline-level=\"3\">Title &amp; more</text:h>"));
    assert!(content.contains(
        "<text:list><text:list-item><text:p>item &lt;one&gt;</text:p></text:list-item></text:list>"
    ));
    assert!(content.contains("<text:p>raw kept</text:p>"));
    assert!(content.contains("<text:p>quoted</text:p>"));

    let again = import_odt_rich(&out).expect("reimport");
    let texts: Vec<&str> = again.blocks.iter().map(|b| b.text.as_str()).collect();
    assert_eq!(texts[0], "first");
    assert_eq!(texts[1], "Title & more");
    assert_eq!(again.blocks[1].kind, BlockKind::Heading { level: 3 });
    assert_eq!(texts[3], "raw kept");
    assert_eq!(texts[4], "quoted");
}

#[test]
fn a_ragged_table_is_padded_to_its_widest_row() {
    let base = odt("<text:p>lead</text:p>");
    let mut model = import_odt_rich(&base).expect("import");
    let cell = |text: &str| RichText::of(vec![Block::new("c", BlockKind::Paragraph, text)]);
    let table = BlockKind::table(vec![
        vec![cell("a"), cell("b"), cell("c")],
        vec![cell("d")],
        vec![RichText::of(Vec::new()), cell("e")],
    ]);
    model.blocks.push(Block::new("t", table, ""));
    let out = materialise_odt_rich(&base, &model).expect("materialise");
    let content = part(&out, "content.xml");
    assert!(content.contains("<table:table-column table:number-columns-repeated=\"3\"/>"));
    assert_eq!(content.matches("<table:table-row>").count(), 3);
    assert_eq!(
        content
            .matches("<table:table-cell office:value-type=\"string\">")
            .count(),
        9
    );
    assert_eq!(content.matches("<text:p/>").count(), 4);
    assert!(content.contains("<text:p>e</text:p>"));
}

#[test]
fn an_unchanged_model_returns_the_base_bytes_and_a_broken_package_is_refused() {
    let base = odt("<text:p>same</text:p>");
    let model = import_odt_rich(&base).expect("import");
    assert_eq!(materialise_odt_rich(&base, &model).expect("same"), base);
    assert!(import_odt_rich(b"not a zip").is_err());
    assert!(materialise_odt_rich(b"not a zip", &model).is_err());
}
