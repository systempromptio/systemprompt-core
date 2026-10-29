//! Allow-list checks on SVG attribute values, `href` targets and
//! stylesheets, applied after character references are resolved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{INERT_NS, unsafe_svg};
use crate::uploads::error::UploadResult;

const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";

const DATA_IMAGES: [&str; 4] = [
    "data:image/png",
    "data:image/jpeg",
    "data:image/gif",
    "data:image/webp",
];

const CSS_FORBIDDEN: [&str; 13] = [
    "\\",
    "@import",
    "@font-face",
    "@namespace",
    "expression(",
    "javascript:",
    "vbscript:",
    "-moz-binding",
    "behavior",
    "image-set(",
    "image(",
    "cross-fade(",
    "src(",
];

pub(super) fn check_attribute(
    element: &str,
    namespace: Option<&str>,
    name: &str,
    value: &str,
) -> UploadResult<()> {
    if name
        .get(..2)
        .is_some_and(|head| head.eq_ignore_ascii_case("on"))
    {
        return Err(unsafe_svg(format!("the event handler {name}")));
    }
    match namespace {
        None | Some(XLINK_NS) => {},
        Some(XML_NS) if name != "base" => {},
        Some(namespace) if INERT_NS.contains(&namespace) => {},
        Some(namespace) => {
            return Err(unsafe_svg(format!(
                "the attribute {name} of the {namespace} namespace"
            )));
        },
    }
    if name == "href" {
        return check_href(element, value);
    }
    check_css(value)
}

fn url_form(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace() && !ch.is_control())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn check_href(element: &str, value: &str) -> UploadResult<()> {
    let target = url_form(value);
    if target.starts_with('#') {
        return Ok(());
    }
    let inline_picture = element == "image"
        && DATA_IMAGES.iter().any(|prefix| {
            target
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with([';', ',']))
        });
    if inline_picture {
        return Ok(());
    }
    Err(unsafe_svg(format!(
        "a reference to {value:?} that is not a #fragment"
    )))
}

pub(super) fn check_css(sheet: &str) -> UploadResult<()> {
    let compact = url_form(sheet);
    if let Some(found) = CSS_FORBIDDEN
        .iter()
        .find(|needle| compact.contains(**needle))
    {
        return Err(unsafe_svg(format!("the stylesheet construct {found}")));
    }
    let mut rest = compact.as_str();
    while let Some(at) = rest.find("url(") {
        rest = &rest[at + 4..];
        let target = rest.trim_start_matches(['"', '\'']);
        if !target.starts_with('#') {
            return Err(unsafe_svg("a stylesheet url() that is not a #fragment"));
        }
    }
    Ok(())
}
