use systemprompt_files::documents::codecs::pptx::{deck_model, lower_deck};
use systemprompt_files::documents::engine::loro::deck::DeckModel;
use systemprompt_files::documents::model::{Mark, MarkKind};

use super::pptx_deck_model::{FRAMED, shape};
use super::pptx_fixture::{framed_deck, part, rebuild};

const NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"";
const GROUP: &str =
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>";

const RICH_RPR: &str = "<a:rPr lang=\"de-DE\" b=\"true\" u=\"none\" strike=\"noStrike\" baseline=\"0\" sz=\"1050\" dirty=\"0\"><a:ln w=\"1\"/><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:effectLst/><a:uLn/><a:ea typeface=\"E\"/><a:cs typeface=\"C\"/><a:sym typeface=\"S\"/><a:hlinkClick r:id=\"\"/><a:rtl/><a:extLst/></a:rPr>";

fn slide_with(paragraphs: &str) -> Vec<u8> {
    let sp = format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Box\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"
    );
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<p:sld {NS}><p:cSld><p:spTree>{GROUP}{sp}</p:spTree></p:cSld></p:sld>"
    );
    rebuild(&framed_deck(), &[(FRAMED, xml)])
}

fn rich_deck() -> (Vec<u8>, DeckModel) {
    let bytes = slide_with(&format!(
        "<a:p><a:r>{RICH_RPR}<a:t>Wide</a:t></a:r><a:r><a:t></a:t></a:r><a:br/><a:fld id=\"{{1}}\" type=\"slidenum\"><a:rPr i=\"1\" sz=\"2400\"/><a:t>7</a:t></a:fld></a:p>"
    ));
    let deck = deck_model(&bytes).expect("deck");
    (bytes, deck)
}

fn keys(marks: &[Mark], start: usize) -> Vec<String> {
    let mut found: Vec<String> = marks
        .iter()
        .filter(|mark| mark.start == start)
        .map(|mark| format!("{}@{}", mark.kind.key(), mark.end))
        .collect();
    found.sort();
    found
}

#[test]
fn run_properties_read_only_the_marks_they_switch_on() {
    let (_, deck) = rich_deck();
    let para = &shape(&deck, FRAMED, "2").paragraphs[0];
    assert_eq!(para.text, "Wide\n7");
    assert_eq!(keys(&para.marks, 0), ["bold@4", "size@4"]);
    assert!(para.marks.contains(&Mark {
        start: 0,
        end: 4,
        kind: MarkKind::Size {
            value: "10.5".to_owned()
        }
    }));
    assert_eq!(keys(&para.marks, 5), ["italic@6", "size@6"]);
    assert!(para.marks.contains(&Mark {
        start: 5,
        end: 6,
        kind: MarkKind::Size {
            value: "24".to_owned()
        }
    }));
}

#[test]
fn written_marks_keep_foreign_run_children_in_schema_order() {
    let (bytes, mut deck) = rich_deck();
    let target = deck
        .slides
        .iter_mut()
        .find(|slide| slide.id == FRAMED)
        .and_then(|slide| slide.shapes.iter_mut().find(|sh| sh.id == "2"))
        .expect("shape");
    let para = &mut target.paragraphs[0];
    para.marks = vec![
        Mark {
            start: 0,
            end: 4,
            kind: MarkKind::Color {
                value: "#12ab34".to_owned(),
            },
        },
        Mark {
            start: 0,
            end: 4,
            kind: MarkKind::Highlight {
                value: "#abc".to_owned(),
            },
        },
        Mark {
            start: 0,
            end: 4,
            kind: MarkKind::Size {
                value: "10.5".to_owned(),
            },
        },
        Mark {
            start: 0,
            end: 4,
            kind: MarkKind::Font {
                value: "Mono".to_owned(),
            },
        },
        Mark {
            start: 5,
            end: 6,
            kind: MarkKind::Size {
                value: "9999".to_owned(),
            },
        },
        Mark {
            start: 5,
            end: 6,
            kind: MarkKind::Code,
        },
    ];
    let out = lower_deck(&bytes, &deck).expect("lower");
    let xml = part(&out, FRAMED);
    let ln = xml.find("<a:ln w=\"1\"/>").expect("ln kept");
    let fill = xml.find("<a:srgbClr val=\"12AB34\"/>").expect("colour");
    let effect = xml.find("<a:effectLst/>").expect("effects kept");
    let latin = xml.find("<a:latin typeface=\"Mono\"/>").expect("font");
    let ea = xml.find("<a:ea typeface=\"E\"/>").expect("ea kept");
    assert!(ln < fill && fill < effect && effect < latin && latin < ea);
    assert_eq!(xml.matches("<a:srgbClr val=\"12AB34\"/>").count(), 1);
    assert!(!xml.contains("val=\"ABC\""));
    assert!(xml.contains("lang=\"de-DE\""));
    assert!(xml.contains("dirty=\"0\""));
    assert!(xml.contains("sz=\"1050\""));
    assert!(!xml.contains("sz=\"999900\""));

    let read = shape(&deck_model(&out).expect("deck"), FRAMED, "2").paragraphs[0].clone();
    assert_eq!(read.text, "Wide\n7");
    assert_eq!(keys(&read.marks, 0), ["color@4", "font@4", "size@4"]);
    assert!(keys(&read.marks, 5).is_empty());
}

#[test]
fn a_line_break_in_rewritten_text_becomes_a_break_with_the_run_style() {
    let bytes = slide_with("<a:p><a:r><a:rPr b=\"1\"/><a:t>One</a:t></a:r></a:p>");
    let mut deck = deck_model(&bytes).expect("deck");
    let para = &mut deck
        .slides
        .iter_mut()
        .find(|slide| slide.id == FRAMED)
        .expect("slide")
        .shapes[0]
        .paragraphs[0];
    para.text = "One\nTwo".to_owned();
    para.marks = vec![Mark {
        start: 0,
        end: 7,
        kind: MarkKind::Bold,
    }];
    let out = lower_deck(&bytes, &deck).expect("lower");
    let read = shape(&deck_model(&out).expect("deck"), FRAMED, "2").clone();
    let texts: Vec<&str> = read.paragraphs.iter().map(|p| p.text.as_str()).collect();
    assert_eq!(texts, ["One\nTwo"]);
    assert!(part(&out, FRAMED).contains("<a:br><a:rPr"));
}
