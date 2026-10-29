//! SVG uploads are parsed as XML and refused, not sanitised, unless every
//! element and attribute is on an allow-list of inert drawing content.
//!
//! Elements must be SVG drawing, structure, paint-server, filter or text
//! elements, or belong to an editor-metadata namespace (Inkscape,
//! Sodipodi, RDF, Creative Commons, Dublin Core) that no renderer acts on.
//! Scripts, `foreignObject`, animation (which can rewrite any attribute),
//! embedded documents and elements of any other namespace are refused, as
//! are processing instructions, an internal DTD subset and entity
//! references other than the predefined and character ones.
//!
//! Attribute values are checked after the parser has resolved character
//! references and after every whitespace and control character a browser
//! ignores in a URL is removed: an `href` must name a `#fragment` (an
//! `image` may also hold an inline `data:image/png|jpeg|gif|webp` picture),
//! no attribute may be an event handler or set `xml:base`, and every
//! stylesheet (a `style` attribute or element, or a presentation
//! attribute) may reference only `url(#fragment)`, with no escapes,
//! imports or URL-taking image functions. A plain vector drawing passes
//! unchanged. The content route still serves an SVG as an attachment under
//! a sandbox CSP with `nosniff`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{QName, ResolveResult};
use quick_xml::{NsReader, XmlVersion};

use crate::uploads::error::{RefusalReason, SvgParseError, UploadError, UploadResult};

mod allow;

use allow::{check_attribute, check_css};

const SVG_NS: &str = "http://www.w3.org/2000/svg";

const INERT_NS: [&str; 6] = [
    "http://www.inkscape.org/namespaces/inkscape",
    "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd",
    "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
    "http://creativecommons.org/ns#",
    "http://purl.org/dc/elements/1.1/",
    "http://purl.org/dc/terms/",
];

const ELEMENTS: [&str; 60] = [
    "svg",
    "g",
    "defs",
    "symbol",
    "use",
    "a",
    "switch",
    "view",
    "title",
    "desc",
    "metadata",
    "style",
    "image",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "textPath",
    "linearGradient",
    "radialGradient",
    "stop",
    "pattern",
    "clipPath",
    "mask",
    "marker",
    "filter",
    "feBlend",
    "feColorMatrix",
    "feComponentTransfer",
    "feComposite",
    "feConvolveMatrix",
    "feDiffuseLighting",
    "feDisplacementMap",
    "feDistantLight",
    "feDropShadow",
    "feFlood",
    "feFuncA",
    "feFuncB",
    "feFuncG",
    "feFuncR",
    "feGaussianBlur",
    "feMerge",
    "feMergeNode",
    "feMorphology",
    "feOffset",
    "fePointLight",
    "feSpecularLighting",
    "feSpotLight",
    "feTile",
    "feTurbulence",
    "hatch",
    "hatchpath",
    "solidcolor",
    "mesh",
    "meshgradient",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Space {
    Svg,
    Inert,
}

pub(crate) fn check(bytes: &[u8]) -> UploadResult<()> {
    let text =
        std::str::from_utf8(bytes).map_err(|error| unreadable(SvgParseError::Utf8(error)))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = NsReader::from_str(text);
    let mut style: Option<String> = None;
    let mut saw_root = false;
    loop {
        let (space, event) = {
            let (resolved, event) = reader
                .read_resolved_event()
                .map_err(|error| unreadable(SvgParseError::Xml(error)))?;
            (element_space(&resolved, &event)?, event)
        };
        match event {
            Event::Start(start) => {
                if style.is_some() {
                    return Err(unsafe_svg("a style element holds markup"));
                }
                check_element(&reader, space, &start)?;
                saw_root = true;
                if space == Some(Space::Svg) && start.local_name().as_ref() == "style" {
                    style = Some(String::new());
                }
            },
            Event::Empty(start) => {
                if style.is_some() {
                    return Err(unsafe_svg("a style element holds markup"));
                }
                check_element(&reader, space, &start)?;
                saw_root = true;
            },
            Event::End(_) => {
                if let Some(sheet) = style.take() {
                    check_css(&sheet)?;
                }
            },
            Event::Text(content) => {
                if let Some(sheet) = style.as_mut() {
                    sheet.push_str(&content.into_inner());
                }
            },
            Event::CData(content) => {
                if let Some(sheet) = style.as_mut() {
                    sheet.push_str(&content.into_inner());
                }
            },
            Event::GeneralRef(reference) => {
                let resolved = resolve_reference(&reference)?;
                if let Some(sheet) = style.as_mut() {
                    sheet.push_str(&resolved);
                }
            },
            Event::DocType(doctype) => {
                let declared = doctype.into_inner().to_ascii_lowercase();
                if declared.contains('[') || declared.contains("entity") {
                    return Err(unsafe_svg("it declares an internal DTD subset"));
                }
            },
            Event::PI(_) => return Err(unsafe_svg("it carries a processing instruction")),
            Event::Decl(_) | Event::Comment(_) => {},
            Event::Eof => break,
        }
    }
    if saw_root {
        Ok(())
    } else {
        Err(unsafe_svg("it holds no element"))
    }
}

fn unsafe_svg(detail: impl std::fmt::Display) -> UploadError {
    UploadError::refused(
        RefusalReason::UnsafeSvg,
        format!("the SVG carries active or external content: {detail}"),
    )
}

fn unreadable(cause: SvgParseError) -> UploadError {
    UploadError::Refused {
        reason: RefusalReason::UnsafeSvg,
        detail: format!("the SVG cannot be read: {cause}"),
        cause: Some(cause),
    }
}

fn element_space(resolved: &ResolveResult<'_>, event: &Event<'_>) -> UploadResult<Option<Space>> {
    if !matches!(event, Event::Start(_) | Event::Empty(_)) {
        return Ok(None);
    }
    match resolved {
        ResolveResult::Unbound => Ok(Some(Space::Svg)),
        ResolveResult::Bound(namespace) if namespace.as_ref() == SVG_NS => Ok(Some(Space::Svg)),
        ResolveResult::Bound(namespace) if INERT_NS.contains(&namespace.as_ref()) => {
            Ok(Some(Space::Inert))
        },
        ResolveResult::Bound(namespace) => Err(unsafe_svg(format!(
            "an element of the {} namespace",
            namespace.as_ref()
        ))),
        ResolveResult::Unknown(prefix) => Err(unsafe_svg(format!(
            "an element with the unbound prefix {prefix}"
        ))),
    }
}

fn check_element(
    reader: &NsReader<&[u8]>,
    space: Option<Space>,
    start: &BytesStart<'_>,
) -> UploadResult<()> {
    let local = start.local_name();
    let local = local.as_ref();
    if space == Some(Space::Svg) && !ELEMENTS.contains(&local) {
        return Err(unsafe_svg(format!("a <{local}> element")));
    }
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| unreadable(SvgParseError::Attribute(error)))?;
        if is_namespace_declaration(attribute.key) {
            continue;
        }
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|error| unreadable(SvgParseError::AttributeValue(error)))?;
        let (resolved, name) = reader.resolver().resolve_attribute(attribute.key);
        let name = name.as_ref();
        let namespace = match &resolved {
            ResolveResult::Unbound => None,
            ResolveResult::Bound(namespace) => Some(namespace.as_ref()),
            ResolveResult::Unknown(prefix) => {
                return Err(unsafe_svg(format!(
                    "an attribute with the unbound prefix {prefix}"
                )));
            },
        };
        check_attribute(local, namespace, name, &value)?;
    }
    Ok(())
}

fn is_namespace_declaration(key: QName<'_>) -> bool {
    let raw = key.as_ref();
    raw == "xmlns" || raw.starts_with("xmlns:")
}

fn resolve_reference(reference: &quick_xml::events::BytesRef<'_>) -> UploadResult<String> {
    if let Some(ch) = reference
        .resolve_char_ref()
        .map_err(|error| unreadable(SvgParseError::CharRef(error)))?
    {
        return Ok(ch.to_string());
    }
    let name: &str = reference;
    resolve_predefined_entity(name)
        .map(str::to_owned)
        .ok_or_else(|| unsafe_svg(format!("the entity reference &{name};")))
}
