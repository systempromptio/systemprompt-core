use systemprompt_files::documents::model::layout::CalloutTone;
use systemprompt_files::documents::model::markdown::{degrade, import, materialise};
use systemprompt_files::documents::model::{Block, BlockKind, Mark, MarkKind, RichText};
use systemprompt_provider_contracts::documents::CodecError;

const GFM: &str = "---\ntitle: Plan\n---\n\n# Plan\n\n- [x] Draft\n- [ ] Review\n\n\
> [!WARNING]\n> Figures are provisional.\n\n```rust\nfn main() {}\n```\n\n\
| Region | Total |\n| --- | --- |\n| EMEA | 9 |\n\n\
![Chart](https://cdn.example.com/c.png \"Figure 1\")\n\n\
Growth<sup>1</sup> was <u>strong</u>[^n].\n\n[^n]: Unaudited.\n";

#[test]
fn gfm_imports_every_construct() {
    let model = import(GFM);
    let settings = model.settings();
    assert_eq!(settings.front_matter.as_deref(), Some("title: Plan\n"));
    let kinds: Vec<&BlockKind> = model.blocks.iter().map(|block| &block.kind).collect();
    assert!(kinds.contains(&&BlockKind::ListItem {
        level: 0,
        ordered: false,
        numbering: None,
        checked: Some(true),
    }));
    assert!(kinds.contains(&&BlockKind::Callout {
        tone: CalloutTone::Warning,
    }));
    assert!(kinds.contains(&&BlockKind::Code {
        language: Some("rust".to_owned()),
    }));
    assert!(kinds.iter().any(|kind| matches!(
        kind,
        BlockKind::Image { caption: Some(caption), .. } if caption == "Figure 1"
    )));
    assert!(kinds.iter().any(|kind| matches!(
        kind,
        BlockKind::Table { props, .. } if props.header_rows == 1
    )));
    assert!(kinds.iter().any(|kind| matches!(
        kind,
        BlockKind::Footnote { label } if label == "n"
    )));
    let growth = model
        .blocks
        .iter()
        .find(|block| block.text.starts_with("Growth"))
        .expect("growth paragraph");
    let keys: Vec<&str> = growth.marks.iter().map(|mark| mark.kind.key()).collect();
    assert!(keys.contains(&"superscript"));
    assert!(keys.contains(&"underline"));
    assert!(keys.contains(&"footnote"));
}

#[test]
fn gfm_materialises_and_imports_to_the_same_model() {
    let model = import(GFM);
    let written = materialise(&model).expect("materialise");
    assert_eq!(import(&written), model);
}

fn coloured() -> RichText {
    let mut block = Block::new("b1", BlockKind::Paragraph, "red text");
    block.marks.push(Mark {
        start: 0,
        end: 3,
        kind: MarkKind::Color {
            value: "#FF0000".to_owned(),
        },
    });
    RichText::of(vec![block, Block::new("b2", BlockKind::PageBreak, "")])
}

#[test]
fn markdown_refuses_what_it_cannot_hold_by_name() {
    let error = materialise(&coloured()).expect_err("refused");
    assert!(matches!(&error, CodecError::Refused { detail } if detail.contains("b1")));
}

#[test]
fn a_live_checkpoint_degrades_instead_of_refusing() {
    let (degraded, warnings) = degrade(&coloured());
    assert_eq!(warnings.len(), 2);
    assert_eq!(degraded.blocks[0].text, "red text");
    assert!(degraded.blocks[0].marks.is_empty());
    assert_eq!(degraded.blocks[1].kind, BlockKind::Divider);
    assert!(materialise(&degraded).is_ok());
}
