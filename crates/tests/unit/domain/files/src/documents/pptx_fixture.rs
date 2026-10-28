use std::io::{Cursor, Read, Write};

use systemprompt_files::documents::codecs::pptx::{PptxCodec, blank_pptx};
use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{DocumentCodec, EditOp, Projection};
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

const NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"";
const GROUP: &str =
    "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub fn entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("zip");
    (0..archive.len())
        .map(|index| {
            let mut file = archive.by_index(index).expect("entry");
            let mut out = Vec::new();
            file.read_to_end(&mut out).expect("read");
            (file.name().expect("name").into_owned(), out)
        })
        .collect()
}

pub fn raw_entries(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).expect("zip");
    (0..archive.len())
        .map(|index| {
            let mut file = archive.by_index_raw(index).expect("entry");
            let mut out = Vec::new();
            file.read_to_end(&mut out).expect("read");
            (file.name().expect("name").into_owned(), out)
        })
        .collect()
}

pub fn part(bytes: &[u8], name: &str) -> String {
    let (_, raw) = entries(bytes)
        .into_iter()
        .find(|(entry, _)| entry == name)
        .expect("part");
    String::from_utf8(raw).expect("utf8")
}

pub fn rebuild(bytes: &[u8], parts: &[(&str, String)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default();
    let mut existing = entries(bytes);
    for (name, xml) in parts {
        match existing.iter_mut().find(|(entry, _)| entry == name) {
            Some(entry) => entry.1 = xml.as_bytes().to_vec(),
            None => existing.push(((*name).to_owned(), xml.as_bytes().to_vec())),
        }
    }
    for (name, raw) in existing {
        writer.start_file(name, options).expect("start");
        writer.write_all(&raw).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

fn sp(id: u32, name: &str, ph: &str, paragraphs: &str) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr/><p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"
    )
}

fn slide(shapes: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<p:sld {NS}><p:cSld><p:spTree>{GROUP}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
    )
}

pub fn agenda_slide() -> String {
    let title = sp(
        2,
        "Title 1",
        "<p:ph type=\"title\"/>",
        "<a:p><a:r><a:rPr lang=\"en-US\" b=\"1\"/><a:t>Agenda</a:t></a:r></a:p>",
    );
    let body = sp(
        3,
        "Content 2",
        "<p:ph idx=\"1\"/>",
        "<a:p><a:r><a:rPr lang=\"en-US\" i=\"1\"/><a:t>Revenue &amp; costs</a:t></a:r></a:p><a:p><a:pPr lvl=\"1\"/><a:r><a:rPr lang=\"en-US\"/><a:t>Hiring</a:t></a:r><a:br/><a:r><a:t>plan</a:t></a:r></a:p>",
    );
    let note = sp(
        4,
        "TextBox 3",
        "",
        "<a:p><a:r><a:t>Dear {{cus</a:t></a:r><a:r><a:t>tomer}}, hello {{name}}</a:t></a:r></a:p>",
    );
    let table = "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"5\" name=\"Table 4\"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\"><a:tbl><a:tr h=\"1\"><a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>Q1</a:t></a:r></a:p></a:txBody></a:tc><a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:t>10</a:t></a:r></a:p></a:txBody></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>";
    let picture = "<p:pic><p:nvPicPr><p:cNvPr id=\"6\" name=\"Picture 5\"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed=\"rId2\"/></p:blipFill><p:spPr/></p:pic>";
    slide(&format!("{title}{body}{note}{table}{picture}"))
}

pub fn intro_slide() -> String {
    slide(&sp(
        2,
        "Title 1",
        "<p:ph type=\"title\"/>",
        "<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>Intro</a:t></a:r></a:p>",
    ))
}

pub fn two_slide_deck() -> Vec<u8> {
    let blank = blank_pptx().expect("blank");
    let presentation = part(&blank, "ppt/presentation.xml").replace(
        "<p:sldId id=\"256\" r:id=\"rId2\"/>",
        "<p:sldId id=\"300\" r:id=\"rId7\"/><p:sldId id=\"256\" r:id=\"rId2\"/>",
    );
    let rels = part(&blank, "ppt/_rels/presentation.xml.rels").replace(
        "</Relationships>",
        &format!("<Relationship Id=\"rId7\" Type=\"{REL}/slide\" Target=\"slides/slide2.xml\"/></Relationships>"),
    );
    let types = part(&blank, "[Content_Types].xml").replace(
        "</Types>",
        "<Override PartName=\"/ppt/slides/slide2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/></Types>",
    );
    let layout_rels = part(&blank, "ppt/slides/_rels/slide1.xml.rels");
    let slide1_rels = layout_rels.replace(
        "</Relationships>",
        &format!("<Relationship Id=\"rId2\" Type=\"{REL}/image\" Target=\"../media/image1.png\"/><Relationship Id=\"rId3\" Type=\"{REL}/notesSlide\" Target=\"../notesSlides/notesSlide1.xml\"/></Relationships>"),
    );
    let notes = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<p:notes {NS}><p:cSld><p:spTree>{GROUP}{}</p:spTree></p:cSld></p:notes>",
        sp(
            2,
            "Notes 1",
            "<p:ph type=\"body\" idx=\"1\"/>",
            "<a:p><a:r><a:t>Speak slowly</a:t></a:r></a:p>"
        )
    );
    rebuild(
        &blank,
        &[
            ("ppt/presentation.xml", presentation),
            ("ppt/_rels/presentation.xml.rels", rels),
            ("[Content_Types].xml", types),
            ("ppt/slides/slide1.xml", agenda_slide()),
            ("ppt/slides/_rels/slide1.xml.rels", slide1_rels),
            ("ppt/slides/slide2.xml", intro_slide()),
            ("ppt/slides/_rels/slide2.xml.rels", layout_rels),
            ("ppt/notesSlides/notesSlide1.xml", notes),
        ],
    )
}

pub fn project(bytes: &[u8]) -> Projection {
    PptxCodec.project(bytes).expect("project")
}

pub fn node(id: &str) -> DocumentNodeId {
    DocumentNodeId::new(id.to_owned())
}

pub fn hash_of(projection: &Projection, id: &str) -> String {
    projection.find(&node(id)).expect("node").hash8.clone()
}

pub fn apply(bytes: &[u8], ops: &[EditOp]) -> Vec<u8> {
    PptxCodec.apply(bytes, &project(bytes), ops).expect("apply")
}
