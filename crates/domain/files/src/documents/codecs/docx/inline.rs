//! Paragraph-content scan: properties, runs, text and embedded objects.
//!
//! A run that carries a drawing, picture, embedded object or markup
//! compatibility block is kept verbatim when its paragraph's text is
//! replaced; plain text runs and the inline containers that hold them
//! (hyperlinks, insertions, smart tags, simple fields) are the removable
//! part of a paragraph.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use systemprompt_provider_contracts::documents::CodecError;

use super::scan::{Child, ParaProps, RunText, Scanner, TextSeg, attr};

const INLINE_CONTAINERS: [&str; 9] = [
    "w:hyperlink",
    "w:ins",
    "w:smartTag",
    "w:fldSimple",
    "w:customXml",
    "w:sdt",
    "w:sdtContent",
    "w:dir",
    "w:bdo",
];

const OBJECTS: [&str; 4] = ["w:drawing", "w:pict", "w:object", "mc:AlternateContent"];

#[derive(Debug, Default)]
pub(super) struct ParaAcc {
    pub(super) ppr: Option<ParaProps>,
    pub(super) children: Vec<Child>,
    pub(super) runs: Vec<RunText>,
    pub(super) texts: Vec<TextSeg>,
    pub(super) first_rpr: Option<Range<usize>>,
    pub(super) images: Vec<String>,
    seen_run: bool,
}

impl ParaAcc {
    pub(super) fn read(&mut self, scanner: &mut Scanner<'_>) -> Result<(), CodecError> {
        loop {
            let (event, range) = scanner.next()?;
            match &event {
                Event::Start(e) if e.name().as_ref() == "w:pPr" => {
                    self.ppr = Some(read_ppr(scanner, range.start)?);
                },
                Event::Empty(e) if e.name().as_ref() == "w:pPr" => {
                    self.ppr = Some(ParaProps {
                        range,
                        ..ParaProps::default()
                    });
                },
                Event::Start(e) if e.name().as_ref() == "w:r" => {
                    let object = self.run(scanner)?;
                    self.push_child(range.start, scanner, !object)?;
                },
                Event::Start(e) if INLINE_CONTAINERS.contains(&e.name().as_ref()) => {
                    let object = self.inline(scanner, e.name().as_ref())?;
                    self.push_child(range.start, scanner, !object)?;
                },
                Event::Start(_) => {
                    self.images.extend(scanner.skip()?);
                    self.push_child(range.start, scanner, false)?;
                },
                Event::Empty(e) => {
                    let removable = e.name().as_ref() == "w:r";
                    self.children.push(Child { range, removable });
                },
                Event::End(e) if e.name().as_ref() == "w:p" => return Ok(()),
                _ => {},
            }
        }
    }

    fn push_child(
        &mut self,
        start: usize,
        scanner: &Scanner<'_>,
        removable: bool,
    ) -> Result<(), CodecError> {
        self.children.push(Child {
            range: start..scanner.pos()?,
            removable,
        });
        Ok(())
    }

    fn inline(&mut self, scanner: &mut Scanner<'_>, end: &str) -> Result<bool, CodecError> {
        let end = end.to_owned();
        let mut object = false;
        loop {
            let (event, _) = scanner.next()?;
            match &event {
                Event::Start(e) if e.name().as_ref() == "w:r" => object |= self.run(scanner)?,
                Event::Start(e) if INLINE_CONTAINERS.contains(&e.name().as_ref()) => {
                    object |= self.inline(scanner, e.name().as_ref())?;
                },
                Event::Start(_) => {
                    scanner.skip()?;
                },
                Event::End(e) if e.name().as_ref() == end.as_str() => return Ok(object),
                _ => {},
            }
        }
    }

    fn run(&mut self, scanner: &mut Scanner<'_>) -> Result<bool, CodecError> {
        let mut run = RunText::default();
        let mut rpr = None;
        let mut object = false;
        loop {
            let (event, range) = scanner.next()?;
            match &event {
                Event::Start(e) if e.name().as_ref() == "w:rPr" => {
                    read_rpr(scanner, &mut run)?;
                    rpr = Some(range.start..scanner.pos()?);
                },
                Event::Empty(e) if e.name().as_ref() == "w:rPr" => rpr = Some(range),
                Event::Start(e) if e.name().as_ref() == "w:t" => {
                    let text = read_text(scanner)?;
                    run.text.push_str(&text);
                    self.texts.push(TextSeg {
                        range: range.start..scanner.pos()?,
                        text,
                    });
                },
                Event::Empty(e) => match e.name().as_ref() {
                    "w:tab" | "w:ptab" => run.text.push('\t'),
                    "w:br" | "w:cr" => run.text.push('\n'),
                    "w:noBreakHyphen" => run.text.push('-'),
                    _ => {},
                },
                Event::Start(e) if OBJECTS.contains(&e.name().as_ref()) => {
                    object = true;
                    self.images.extend(scanner.skip()?);
                },
                Event::Start(_) => {
                    scanner.skip()?;
                },
                Event::End(e) if e.name().as_ref() == "w:r" => break,
                _ => {},
            }
        }
        if !self.seen_run {
            self.seen_run = true;
            self.first_rpr = rpr;
        }
        self.runs.push(run);
        Ok(object)
    }
}

fn read_ppr(scanner: &mut Scanner<'_>, start: usize) -> Result<ParaProps, CodecError> {
    let mut props = ParaProps::default();
    loop {
        let (event, range) = scanner.next()?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                let open = matches!(event, Event::Start(_));
                let name = e.name().as_ref().to_owned();
                let style = (name == "w:pStyle").then(|| attr(e, "w:val")).flatten();
                if open {
                    scanner.skip()?;
                }
                let full = range.start..scanner.pos()?;
                match name.as_str() {
                    "w:pStyle" => {
                        props.style = style;
                        props.style_range = Some(full);
                    },
                    "w:numPr" => props.num_range = Some(full),
                    _ => {},
                }
            },
            Event::End(e) if e.name().as_ref() == "w:pPr" => {
                props.range = start..scanner.pos()?;
                return Ok(props);
            },
            _ => {},
        }
    }
}

fn read_rpr(scanner: &mut Scanner<'_>, run: &mut RunText) -> Result<(), CodecError> {
    loop {
        let (event, _) = scanner.next()?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                let on = toggle_on(e);
                match e.name().as_ref() {
                    "w:b" => run.bold = on,
                    "w:i" => run.italic = on,
                    "w:u" => run.underline = on,
                    _ => {},
                }
                if matches!(event, Event::Start(_)) {
                    scanner.skip()?;
                }
            },
            Event::End(e) if e.name().as_ref() == "w:rPr" => return Ok(()),
            _ => {},
        }
    }
}

fn toggle_on(e: &BytesStart<'_>) -> bool {
    !matches!(
        attr(e, "w:val").as_deref(),
        Some("0" | "false" | "off" | "none")
    )
}

fn read_text(scanner: &mut Scanner<'_>) -> Result<String, CodecError> {
    let mut text = String::new();
    loop {
        let (event, _) = scanner.next()?;
        match &event {
            Event::Text(t) => text.push_str(&t.xml10_content()),
            Event::CData(t) => text.push_str(t.as_ref()),
            Event::GeneralRef(r) => {
                let resolved = r.resolve_char_ref().map_err(|source| CodecError::Parse {
                    part: super::parts::DOCUMENT.to_owned(),
                    source: Box::new(source),
                })?;
                if let Some(ch) = resolved {
                    text.push(ch);
                } else {
                    let name = r.xml10_content();
                    text.push_str(resolve_predefined_entity(&name).unwrap_or_default());
                }
            },
            Event::End(e) if e.name().as_ref() == "w:t" => return Ok(text),
            Event::Start(_) => {
                scanner.skip()?;
            },
            _ => {},
        }
    }
}
