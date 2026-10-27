//! A minimal, PowerPoint-openable deck written part by part: one slide
//! master, one "Title and Content" layout, one theme, and one slide holding
//! an empty title placeholder.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io::{Cursor, Write};

use systemprompt_provider_contracts::documents::CodecError;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use super::package::parse_error;

const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n";
const NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PML: &str = "application/vnd.openxmlformats-officedocument.presentationml";
const GROUP: &str = "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>";

fn rels(entries: &[(&str, &str, &str)]) -> String {
    let body: String = entries
        .iter()
        .map(|(id, kind, target)| {
            format!("<Relationship Id=\"{id}\" Type=\"{kind}\" Target=\"{target}\"/>")
        })
        .collect();
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{body}</Relationships>"
    )
}

fn placeholder(id: u32, name: &str, ph: &str, frame: &str, text: &str) -> String {
    let paragraph = if text.is_empty() {
        "<a:p><a:endParaRPr lang=\"en-US\"/></a:p>".to_owned()
    } else {
        format!("<a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{text}</a:t></a:r></a:p>")
    };
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{id}\" name=\"{name}\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr>{ph}</p:nvPr></p:nvSpPr><p:spPr>{frame}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/>{paragraph}</p:txBody></p:sp>"
    )
}

const TITLE_FRAME: &str = "<a:xfrm><a:off x=\"838200\" y=\"365125\"/><a:ext cx=\"10515600\" cy=\"1325563\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>";
const BODY_FRAME: &str = "<a:xfrm><a:off x=\"838200\" y=\"1825625\"/><a:ext cx=\"10515600\" cy=\"4351338\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>";
const TITLE_PH: &str = "<p:ph type=\"title\"/>";
const BODY_PH: &str = "<p:ph idx=\"1\"/>";

fn content_types() -> String {
    let overrides = [
        (
            "/ppt/presentation.xml",
            format!("{PML}.presentation.main+xml"),
        ),
        (
            "/ppt/slideMasters/slideMaster1.xml",
            format!("{PML}.slideMaster+xml"),
        ),
        (
            "/ppt/slideLayouts/slideLayout1.xml",
            format!("{PML}.slideLayout+xml"),
        ),
        ("/ppt/slides/slide1.xml", format!("{PML}.slide+xml")),
        (
            "/ppt/theme/theme1.xml",
            "application/vnd.openxmlformats-officedocument.theme+xml".to_owned(),
        ),
        ("/ppt/presProps.xml", format!("{PML}.presProps+xml")),
        ("/ppt/viewProps.xml", format!("{PML}.viewProps+xml")),
        ("/ppt/tableStyles.xml", format!("{PML}.tableStyles+xml")),
        (
            "/docProps/app.xml",
            "application/vnd.openxmlformats-officedocument.extended-properties+xml".to_owned(),
        ),
    ];
    let body: String = overrides
        .iter()
        .map(|(part, kind)| format!("<Override PartName=\"{part}\" ContentType=\"{kind}\"/>"))
        .collect();
    format!(
        "{DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>{body}</Types>"
    )
}

fn presentation() -> String {
    format!(
        "{DECL}<p:presentation {NS} saveSubsetFonts=\"1\"><p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id=\"256\" r:id=\"rId2\"/></p:sldIdLst><p:sldSz cx=\"12192000\" cy=\"6858000\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/></p:presentation>"
    )
}

fn master() -> String {
    let shapes = format!(
        "{}{}",
        placeholder(
            2,
            "Title Placeholder 1",
            TITLE_PH,
            TITLE_FRAME,
            "Click to edit Master title style"
        ),
        placeholder(
            3,
            "Text Placeholder 2",
            "<p:ph type=\"body\" idx=\"1\"/>",
            BODY_FRAME,
            "Click to edit Master text styles"
        ),
    );
    format!(
        "{DECL}<p:sldMaster {NS}><p:cSld><p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg><p:spTree>{GROUP}{shapes}</p:spTree></p:cSld><p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/><p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle><a:lvl1pPr algn=\"l\"><a:defRPr sz=\"4400\" kern=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mj-lt\"/></a:defRPr></a:lvl1pPr></p:titleStyle><p:bodyStyle><a:lvl1pPr marL=\"228600\" indent=\"-228600\"><a:buFont typeface=\"Arial\"/><a:buChar char=\"&#8226;\"/><a:defRPr sz=\"2800\" kern=\"1200\"><a:solidFill><a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mn-lt\"/></a:defRPr></a:lvl1pPr></p:bodyStyle><p:otherStyle><a:lvl1pPr><a:defRPr sz=\"1800\" kern=\"1200\"/></a:lvl1pPr></p:otherStyle></p:txStyles></p:sldMaster>"
    )
}

fn layout() -> String {
    let shapes = format!(
        "{}{}",
        placeholder(
            2,
            "Title 1",
            TITLE_PH,
            "",
            "Click to edit Master title style"
        ),
        placeholder(
            3,
            "Content Placeholder 2",
            BODY_PH,
            "",
            "Click to edit Master text styles"
        ),
    );
    format!(
        "{DECL}<p:sldLayout {NS} type=\"obj\" preserve=\"1\"><p:cSld name=\"Title and Content\"><p:spTree>{GROUP}{shapes}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"
    )
}

fn slide() -> String {
    format!(
        "{DECL}<p:sld {NS}><p:cSld><p:spTree>{GROUP}{}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>",
        placeholder(2, "Title 1", TITLE_PH, "", "")
    )
}

fn theme() -> String {
    let colours = [
        ("dk1", "<a:sysClr val=\"windowText\" lastClr=\"000000\"/>"),
        ("lt1", "<a:sysClr val=\"window\" lastClr=\"FFFFFF\"/>"),
        ("dk2", "<a:srgbClr val=\"44546A\"/>"),
        ("lt2", "<a:srgbClr val=\"E7E6E6\"/>"),
        ("accent1", "<a:srgbClr val=\"4472C4\"/>"),
        ("accent2", "<a:srgbClr val=\"ED7D31\"/>"),
        ("accent3", "<a:srgbClr val=\"A5A5A5\"/>"),
        ("accent4", "<a:srgbClr val=\"FFC000\"/>"),
        ("accent5", "<a:srgbClr val=\"5B9BD5\"/>"),
        ("accent6", "<a:srgbClr val=\"70AD47\"/>"),
        ("hlink", "<a:srgbClr val=\"0563C1\"/>"),
        ("folHlink", "<a:srgbClr val=\"954F72\"/>"),
    ];
    let colours: String = colours
        .iter()
        .map(|(name, value)| format!("<a:{name}>{value}</a:{name}>"))
        .collect();
    let font = |face: &str| {
        format!("<a:latin typeface=\"{face}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/>")
    };
    let fill = "<a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>";
    let line = format!("<a:ln w=\"6350\">{fill}</a:ln>");
    let effect = "<a:effectStyle><a:effectLst/></a:effectStyle>";
    format!(
        "{DECL}<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"Office Theme\"><a:themeElements><a:clrScheme name=\"Office\">{colours}</a:clrScheme><a:fontScheme name=\"Office\"><a:majorFont>{}</a:majorFont><a:minorFont>{}</a:minorFont></a:fontScheme><a:fmtScheme name=\"Office\"><a:fillStyleLst>{fill}{fill}{fill}</a:fillStyleLst><a:lnStyleLst>{line}{line}{line}</a:lnStyleLst><a:effectStyleLst>{effect}{effect}{effect}</a:effectStyleLst><a:bgFillStyleLst>{fill}{fill}{fill}</a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>",
        font("Calibri Light"),
        font("Calibri"),
    )
}

fn parts() -> Vec<(&'static str, String)> {
    let slide_rel = format!("{REL}/slide");
    let layout_rel = format!("{REL}/slideLayout");
    let master_rel = format!("{REL}/slideMaster");
    let theme_rel = format!("{REL}/theme");
    vec![
        ("[Content_Types].xml", content_types()),
        (
            "_rels/.rels",
            rels(&[
                (
                    "rId1",
                    &format!("{REL}/officeDocument"),
                    "ppt/presentation.xml",
                ),
                (
                    "rId2",
                    &format!("{REL}/extended-properties"),
                    "docProps/app.xml",
                ),
            ]),
        ),
        (
            "docProps/app.xml",
            format!(
                "{DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"><Application>systemprompt.io</Application><Slides>1</Slides></Properties>"
            ),
        ),
        ("ppt/presentation.xml", presentation()),
        (
            "ppt/_rels/presentation.xml.rels",
            rels(&[
                ("rId1", &master_rel, "slideMasters/slideMaster1.xml"),
                ("rId2", &slide_rel, "slides/slide1.xml"),
                ("rId3", &format!("{REL}/presProps"), "presProps.xml"),
                ("rId4", &format!("{REL}/viewProps"), "viewProps.xml"),
                ("rId5", &theme_rel, "theme/theme1.xml"),
                ("rId6", &format!("{REL}/tableStyles"), "tableStyles.xml"),
            ]),
        ),
        ("ppt/slideMasters/slideMaster1.xml", master()),
        (
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            rels(&[
                ("rId1", &layout_rel, "../slideLayouts/slideLayout1.xml"),
                ("rId2", &theme_rel, "../theme/theme1.xml"),
            ]),
        ),
        ("ppt/slideLayouts/slideLayout1.xml", layout()),
        (
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            rels(&[("rId1", &master_rel, "../slideMasters/slideMaster1.xml")]),
        ),
        ("ppt/slides/slide1.xml", slide()),
        (
            "ppt/slides/_rels/slide1.xml.rels",
            rels(&[("rId1", &layout_rel, "../slideLayouts/slideLayout1.xml")]),
        ),
        ("ppt/theme/theme1.xml", theme()),
        (
            "ppt/presProps.xml",
            format!("{DECL}<p:presentationPr {NS}/>"),
        ),
        (
            "ppt/viewProps.xml",
            format!("{DECL}<p:viewPr {NS}><p:gridSpacing cx=\"76200\" cy=\"76200\"/></p:viewPr>"),
        ),
        (
            "ppt/tableStyles.xml",
            format!(
                "{DECL}<a:tblStyleLst xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" def=\"{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}\"/>"
            ),
        ),
    ]
}

pub fn blank_pptx() -> Result<Vec<u8>, CodecError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default());
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, xml) in parts() {
        writer
            .start_file(name, options)
            .map_err(|source| parse_error(name, source))?;
        writer.write_all(xml.as_bytes())?;
    }
    Ok(writer
        .finish()
        .map_err(|source| parse_error("zip container", source))?
        .into_inner())
}
