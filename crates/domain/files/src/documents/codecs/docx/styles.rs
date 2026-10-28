//! Read-only lookup of paragraph style ids in `word/styles.xml`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::Reader;
use quick_xml::events::Event;
use systemprompt_provider_contracts::documents::CodecError;

use super::parts::{self, STYLES};
use super::scan::attr;

pub(super) fn heading_style(bytes: &[u8], level: u8) -> Result<Option<String>, CodecError> {
    let Some(raw) = parts::read_part(bytes, STYLES)? else {
        return Ok(None);
    };
    let xml = String::from_utf8_lossy(&raw);
    let wanted_id = format!("Heading{level}");
    let wanted_name = format!("heading {level}");
    let mut reader = Reader::from_str(&xml);
    let mut current: Option<String> = None;
    loop {
        let event = reader.read_event().map_err(|source| CodecError::Parse {
            part: STYLES.to_owned(),
            source: Box::new(source),
        })?;
        match &event {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "w:style" => {
                current = attr(e, "w:styleId");
                if current.as_deref() == Some(wanted_id.as_str()) {
                    return Ok(current);
                }
            },
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "w:name" => {
                let name = attr(e, "w:val").map(|n| n.to_ascii_lowercase());
                if name.as_deref() == Some(wanted_name.as_str()) && current.is_some() {
                    return Ok(current);
                }
            },
            Event::Eof => return Ok(None),
            _ => {},
        }
    }
}

pub(super) fn has_style(bytes: &[u8], id: &str) -> Result<bool, CodecError> {
    let wanted = format!("w:styleId=\"{id}\"");
    Ok(parts::read_part(bytes, STYLES)?
        .is_some_and(|raw| String::from_utf8_lossy(&raw).contains(&wanted)))
}
