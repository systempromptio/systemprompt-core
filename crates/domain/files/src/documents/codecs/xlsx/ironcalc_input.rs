//! Makes a workbook safe to hand to IronCalc's importer.
//!
//! IronCalc 0.9 indexes the first `fonts`, `fills`, `borders`,
//! `cellStyleXfs`, `cellXfs` and `cellStyles` element of `xl/styles.xml`
//! without checking that it exists, so a valid workbook that leaves one out
//! (`umya-spreadsheet` writes no `cellStyleXfs` once a cell is styled)
//! panics the import. Each missing section is written in with the default
//! Excel itself writes, in schema order, before the import; a workbook that
//! has them all passes through unchanged. The import itself runs behind
//! [`guarded`], so any other panic in it is a parse error, not a dropped
//! request.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::panic::{AssertUnwindSafe, catch_unwind};

use systemprompt_provider_contracts::documents::CodecError;

use crate::documents::codecs::zip_package::Package;

const STYLES: &str = "xl/styles.xml";

const SECTIONS: [(&str, Option<&str>); 7] = [
    ("numFmts", None),
    (
        "fonts",
        Some("<fonts count=\"1\"><font><sz val=\"11\"/><name val=\"Calibri\"/></font></fonts>"),
    ),
    (
        "fills",
        Some(concat!(
            "<fills count=\"2\"><fill><patternFill patternType=\"none\"/></fill>",
            "<fill><patternFill patternType=\"gray125\"/></fill></fills>"
        )),
    ),
    (
        "borders",
        Some(concat!(
            "<borders count=\"1\">",
            "<border><left/><right/><top/><bottom/><diagonal/></border></borders>"
        )),
    ),
    (
        "cellStyleXfs",
        Some(concat!(
            "<cellStyleXfs count=\"1\">",
            "<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>"
        )),
    ),
    (
        "cellXfs",
        Some(concat!(
            "<cellXfs count=\"1\">",
            "<xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\" xfId=\"0\"/></cellXfs>"
        )),
    ),
    (
        "cellStyles",
        Some(concat!(
            "<cellStyles count=\"1\">",
            "<cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>"
        )),
    ),
];

/// The byte offset just past the element `tag`, or `None` when absent.
fn element_end(xml: &str, tag: &str) -> Option<usize> {
    let open = format!("<{tag}");
    let start = xml.match_indices(&open).find_map(|(at, _)| {
        let next = xml[at + open.len()..].chars().next()?;
        matches!(next, ' ' | '>' | '/').then_some(at)
    })?;
    let rest = &xml[start..];
    let head_end = rest.find('>')?;
    if rest[..head_end].ends_with('/') {
        return Some(start + head_end + 1);
    }
    let close = format!("</{tag}>");
    rest.find(&close).map(|at| start + at + close.len())
}

fn completed_styles(xml: &str) -> Option<String> {
    let mut out = xml.to_owned();
    let mut changed = false;
    let sheet = out.find("<styleSheet")?;
    let mut after = sheet + out[sheet..].find('>')? + 1;
    for (tag, default) in SECTIONS {
        match element_end(&out, tag) {
            Some(end) => after = end,
            None => {
                if let Some(default) = default {
                    out.insert_str(after, default);
                    after += default.len();
                    changed = true;
                }
            },
        }
    }
    changed.then_some(out)
}

/// The workbook with every styles section IronCalc requires present.
pub(super) fn importable(bytes: &[u8]) -> Result<Vec<u8>, CodecError> {
    let mut package = Package::open(bytes)?;
    let Some(styles) = package.read(STYLES)? else {
        return Ok(bytes.to_vec());
    };
    match completed_styles(&styles) {
        Some(completed) => {
            package.write(STYLES, completed);
            package.finish()
        },
        None => Ok(bytes.to_vec()),
    }
}

/// Runs an IronCalc call, turning a panic inside it into a parse error.
pub(super) fn guarded<T>(
    part: &str,
    call: impl FnOnce() -> Result<T, CodecError>,
) -> Result<T, CodecError> {
    catch_unwind(AssertUnwindSafe(call)).unwrap_or_else(|_| {
        Err(CodecError::Parse {
            part: part.to_owned(),
            source: "the calc engine could not read this workbook".into(),
        })
    })
}
