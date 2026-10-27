//! Event-level scan of `word/document.xml` into byte-addressed blocks.
//!
//! Every block, run and text element keeps its byte range in the source, so
//! edits splice replacement markup into the original text and everything
//! outside a touched range stays byte-identical.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::ops::Range;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use systemprompt_provider_contracts::documents::CodecError;

use super::inline::ParaAcc;
use super::parts::{DOCUMENT, malformed};

#[derive(Debug)]
pub(super) struct Doc<'a> {
    pub(super) src: &'a str,
    pub(super) blocks: Vec<Block>,
}

#[derive(Debug)]
pub(super) enum Block {
    Para(Box<Para>),
    Table(Table),
}

#[derive(Debug, Default)]
pub(super) struct Para {
    pub(super) id: String,
    pub(super) range: Range<usize>,
    pub(super) open_tag: String,
    pub(super) w14_id: Option<String>,
    pub(super) ppr: Option<ParaProps>,
    pub(super) children: Vec<Child>,
    pub(super) runs: Vec<RunText>,
    pub(super) texts: Vec<TextSeg>,
    pub(super) first_rpr: Option<Range<usize>>,
    pub(super) images: Vec<String>,
}

#[derive(Debug, Default)]
pub(super) struct ParaProps {
    pub(super) range: Range<usize>,
    pub(super) style: Option<String>,
    pub(super) style_range: Option<Range<usize>>,
    pub(super) num_range: Option<Range<usize>>,
}

#[derive(Debug)]
pub(super) struct Child {
    pub(super) range: Range<usize>,
    pub(super) removable: bool,
}

#[derive(Debug, Default)]
pub(super) struct RunText {
    pub(super) text: String,
    pub(super) bold: bool,
    pub(super) italic: bool,
    pub(super) underline: bool,
}

#[derive(Debug)]
pub(super) struct TextSeg {
    pub(super) range: Range<usize>,
    pub(super) text: String,
}

#[derive(Debug)]
pub(super) struct Table {
    pub(super) id: String,
    pub(super) range: Range<usize>,
    pub(super) rows: Vec<Vec<Cell>>,
}

#[derive(Debug)]
pub(super) struct Cell {
    pub(super) id: String,
    pub(super) blocks: Vec<Block>,
}

impl Para {
    pub(super) fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

impl Doc<'_> {
    pub(super) fn paragraphs(&self) -> Vec<&Para> {
        let mut out = Vec::new();
        collect_paragraphs(&self.blocks, &mut out);
        out
    }

    pub(super) fn uses_para_ids(&self) -> bool {
        self.paragraphs().iter().any(|p| p.w14_id.is_some())
    }
}

fn collect_paragraphs<'d>(blocks: &'d [Block], out: &mut Vec<&'d Para>) {
    for block in blocks {
        match block {
            Block::Para(para) => out.push(para.as_ref()),
            Block::Table(table) => {
                for cell in table.rows.iter().flatten() {
                    collect_paragraphs(&cell.blocks, out);
                }
            },
        }
    }
}

pub(super) fn parse(src: &str) -> Result<Doc<'_>, CodecError> {
    let mut scanner = Scanner::new(src);
    loop {
        let (event, _) = scanner.next()?;
        if let Event::Start(e) = &event
            && e.name().as_ref() == "w:body"
        {
            let blocks = scanner.blocks("w:body", "body")?;
            return Ok(Doc { src, blocks });
        }
    }
}

pub(super) fn attr(e: &BytesStart<'_>, key: &str) -> Option<String> {
    let attribute = e.try_get_attribute(key).ok().flatten()?;
    attribute
        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
        .ok()
        .map(std::borrow::Cow::into_owned)
}

const BLOCK_CONTAINERS: [&str; 3] = ["w:sdt", "w:sdtContent", "w:customXml"];

pub(super) struct Scanner<'a> {
    reader: Reader<&'a [u8]>,
    pub(super) src: &'a str,
}

impl<'a> Scanner<'a> {
    fn new(src: &'a str) -> Self {
        let mut reader = Reader::from_str(src);
        reader.config_mut().trim_text(false);
        Self { reader, src }
    }

    pub(super) fn pos(&self) -> Result<usize, CodecError> {
        usize::try_from(self.reader.buffer_position())
            .map_err(|_overflow| malformed("word/document.xml is too large"))
    }

    pub(super) fn next(&mut self) -> Result<(Event<'a>, Range<usize>), CodecError> {
        let start = self.pos()?;
        let event = self
            .reader
            .read_event()
            .map_err(|source| CodecError::Parse {
                part: DOCUMENT.to_owned(),
                source: Box::new(source),
            })?;
        if matches!(event, Event::Eof) {
            return Err(malformed("word/document.xml ended inside an open element"));
        }
        Ok((event, start..self.pos()?))
    }

    pub(super) fn skip(&mut self) -> Result<Vec<String>, CodecError> {
        let mut depth = 0usize;
        let mut rel_ids = Vec::new();
        loop {
            let (event, _) = self.next()?;
            match &event {
                Event::Start(e) | Event::Empty(e) => {
                    rel_ids.extend(attr(e, "r:embed").or_else(|| attr(e, "r:id")));
                    if matches!(event, Event::Start(_)) {
                        depth += 1;
                    }
                },
                Event::End(_) if depth == 0 => return Ok(rel_ids),
                Event::End(_) => depth -= 1,
                _ => {},
            }
        }
    }

    fn blocks(&mut self, end: &str, prefix: &str) -> Result<Vec<Block>, CodecError> {
        let mut blocks = Vec::new();
        let (mut paras, mut tables) = (0usize, 0usize);
        loop {
            let (event, range) = self.next()?;
            match &event {
                Event::Start(e) | Event::Empty(e) if e.name().as_ref() == "w:p" => {
                    let id = format!("{prefix}/p[{paras}]");
                    paras += 1;
                    let empty = matches!(event, Event::Empty(_));
                    blocks.push(Block::Para(Box::new(self.paragraph(e, range, empty, id)?)));
                },
                Event::Start(e) if e.name().as_ref() == "w:tbl" => {
                    let id = format!("{prefix}/tbl[{tables}]");
                    tables += 1;
                    blocks.push(Block::Table(self.table(range.start, id)?));
                },
                Event::Start(e) if BLOCK_CONTAINERS.contains(&e.name().as_ref()) => {},
                Event::Start(_) => {
                    self.skip()?;
                },
                Event::End(e) if e.name().as_ref() == end => return Ok(blocks),
                _ => {},
            }
        }
    }

    fn table(&mut self, start: usize, id: String) -> Result<Table, CodecError> {
        let mut rows = Vec::new();
        loop {
            let (event, _) = self.next()?;
            match &event {
                Event::Start(e) if e.name().as_ref() == "w:tr" => {
                    let row_id = format!("{id}/tr[{}]", rows.len());
                    rows.push(self.row(&row_id)?);
                },
                Event::Start(e) if BLOCK_CONTAINERS.contains(&e.name().as_ref()) => {},
                Event::Start(_) => {
                    self.skip()?;
                },
                Event::End(e) if e.name().as_ref() == "w:tbl" => {
                    return Ok(Table {
                        id,
                        range: start..self.pos()?,
                        rows,
                    });
                },
                _ => {},
            }
        }
    }

    fn row(&mut self, row_id: &str) -> Result<Vec<Cell>, CodecError> {
        let mut cells = Vec::new();
        loop {
            let (event, _) = self.next()?;
            match &event {
                Event::Start(e) if e.name().as_ref() == "w:tc" => {
                    let id = format!("{row_id}/tc[{}]", cells.len());
                    let blocks = self.blocks("w:tc", &id)?;
                    cells.push(Cell { id, blocks });
                },
                Event::Start(e) if BLOCK_CONTAINERS.contains(&e.name().as_ref()) => {},
                Event::Start(_) => {
                    self.skip()?;
                },
                Event::End(e) if e.name().as_ref() == "w:tr" => return Ok(cells),
                _ => {},
            }
        }
    }

    fn paragraph(
        &mut self,
        e: &BytesStart<'_>,
        range: Range<usize>,
        empty: bool,
        positional: String,
    ) -> Result<Para, CodecError> {
        let para_id = attr(e, "w14:paraId");
        let raw = &self.src[range.clone()];
        let open_tag = if empty {
            format!("{}>", raw.trim_end_matches("/>").trim_end())
        } else {
            raw.to_owned()
        };
        let id = para_id
            .as_ref()
            .map_or(positional, |pid| format!("pid:{pid}"));
        let mut acc = ParaAcc::default();
        if !empty {
            acc.read(self)?;
        }
        Ok(Para {
            id,
            range: range.start..self.pos()?,
            open_tag,
            w14_id: para_id,
            ppr: acc.ppr,
            children: acc.children,
            runs: acc.runs,
            texts: acc.texts,
            first_rpr: acc.first_rpr,
            images: acc.images,
        })
    }
}
