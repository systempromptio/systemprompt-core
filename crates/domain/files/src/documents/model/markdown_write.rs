//! The `RichText` model written as GitHub-flavoured Markdown.
//!
//! Markdown holds headings, paragraphs, quotes, alerts (callouts), fenced
//! code with its language, lists and task lists, GFM tables with one header
//! row, images with a caption (their title), footnotes, dividers, front
//! matter, and the marks bold, italic, strike, code, links, underline,
//! superscript and subscript (the last three as inline HTML). Anything
//! else (colours, highlights, fonts, sizes, caps, fields, paragraph
//! formatting, page and section breaks, a table of contents, styled or
//! merged tables, sized images, numbering definitions, styles, themes and
//! page setup) is refused with [`CodecError::Refused`] naming the first
//! block that holds it, never dropped. Comment marks are not refused: the
//! comments live in the document's comment store and re-anchor by id.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_provider_contracts::documents::CodecError;

use super::inline::render_markdown;
use super::layout::CalloutTone;
use super::rich_text::{Block, BlockKind, RichText};

const REFUSED_MARKS: [&str; 6] = ["color", "highlight", "font", "size", "caps", "field"];

pub fn materialise(model: &RichText) -> Result<String, CodecError> {
    check_settings(model)?;
    let mut out = String::new();
    if let Some(front) = model.document.as_ref().and_then(|d| d.front_matter.as_deref()) {
        out.push_str("---\n");
        out.push_str(front.trim_end_matches('\n'));
        out.push_str("\n---\n\n");
    }
    let mut body = String::new();
    let mut previous_list = false;
    for block in &model.blocks {
        check_block(block)?;
        let is_list = matches!(block.kind, BlockKind::ListItem { .. });
        if !body.is_empty() {
            body.push_str(if is_list && previous_list { "\n" } else { "\n\n" });
        }
        previous_list = is_list;
        write_block(&mut body, block)?;
    }
    out.push_str(&body);
    if !body.is_empty() {
        out.push('\n');
    }
    Ok(out)
}

fn refused(detail: String) -> CodecError {
    CodecError::Refused { detail }
}

fn check_settings(model: &RichText) -> Result<(), CodecError> {
    let Some(settings) = model.document.as_deref() else {
        return Ok(());
    };
    let owned = [
        (!settings.styles.is_empty(), "named styles"),
        (settings.theme.is_some(), "a theme"),
        (!settings.numbering.is_empty(), "numbering definitions"),
        (settings.section.is_some(), "page setup, headers or footers"),
    ];
    match owned.iter().find(|(held, _)| *held) {
        Some((_, what)) => Err(refused(format!("Markdown cannot hold {what}"))),
        None => Ok(()),
    }
}

fn check_block(block: &Block) -> Result<(), CodecError> {
    let id = &block.id;
    if !block.attrs.is_default() {
        return Err(refused(format!(
            "block {id} has paragraph formatting (style, alignment, indent or spacing), which \
             Markdown cannot hold"
        )));
    }
    if let Some(mark) = block
        .marks
        .iter()
        .find(|mark| REFUSED_MARKS.contains(&mark.kind.key()))
    {
        return Err(refused(format!(
            "block {id} has a {} mark, which Markdown cannot hold",
            mark.kind.key()
        )));
    }
    let what = match &block.kind {
        BlockKind::ListItem {
            numbering: Some(_), ..
        } => Some("a numbering definition"),
        BlockKind::Table { props, .. } => {
            let plain = props.columns.iter().all(|c| c.width.is_none())
                && props.header_rows <= 1
                && props.style.is_none()
                && props.cells.iter().flatten().all(|cell| *cell == Default::default());
            (!plain).then_some("table widths, styles, shading or merged cells")
        },
        BlockKind::Image {
            width, height, wrap, ..
        } => (width.is_some() || height.is_some() || *wrap != Default::default())
            .then_some("an image size or wrap"),
        BlockKind::PageBreak => Some("a page break"),
        BlockKind::SectionBreak { .. } => Some("a section break"),
        BlockKind::Toc { .. } => Some("a table of contents"),
        _ => None,
    };
    match what {
        Some(what) => Err(refused(format!("block {id} holds {what}, which Markdown cannot hold"))),
        None => Ok(()),
    }
}

fn write_block(out: &mut String, block: &Block) -> Result<(), CodecError> {
    let inline = || render_markdown(&block.text, &block.marks);
    match &block.kind {
        BlockKind::Paragraph => out.push_str(&inline()),
        BlockKind::Heading { level } => {
            out.push_str(&"#".repeat(usize::from(*level)));
            out.push(' ');
            out.push_str(&inline());
        },
        BlockKind::ListItem {
            level,
            ordered,
            checked,
            ..
        } => {
            out.push_str(&"  ".repeat(usize::from(*level)));
            out.push_str(if *ordered { "1. " } else { "- " });
            match checked {
                Some(true) => out.push_str("[x] "),
                Some(false) => out.push_str("[ ] "),
                None => {},
            }
            out.push_str(&inline());
        },
        BlockKind::Quote => {
            out.push_str("> ");
            out.push_str(&inline().replace('\n', "\n> "));
        },
        BlockKind::Callout { tone } => {
            out.push_str(&format!("> [!{}]\n> ", alert(*tone)));
            out.push_str(&inline().replace('\n', "\n> "));
        },
        BlockKind::Footnote { label } => {
            out.push_str(&format!("[^{label}]: "));
            out.push_str(&inline());
        },
        BlockKind::Code { language } => {
            out.push_str("```");
            out.push_str(language.as_deref().unwrap_or(""));
            out.push('\n');
            out.push_str(&block.text);
            out.push_str("\n```");
        },
        BlockKind::Table { rows, .. } => write_table(out, rows),
        BlockKind::Image {
            blob, alt, caption, ..
        } => match caption {
            Some(caption) => {
                let title = caption.replace('"', "\\\"");
                out.push_str(&format!("![{alt}]({blob} \"{title}\")"));
            },
            None => out.push_str(&format!("![{alt}]({blob})")),
        },
        BlockKind::Divider => out.push_str("---"),
        BlockKind::Raw { source } => out.push_str(source),
        BlockKind::PageBreak | BlockKind::SectionBreak { .. } | BlockKind::Toc { .. } => {
            return Err(refused(format!("block {} cannot be written as Markdown", block.id)));
        },
    }
    Ok(())
}

const fn alert(tone: CalloutTone) -> &'static str {
    match tone {
        CalloutTone::Note => "NOTE",
        CalloutTone::Success => "TIP",
        CalloutTone::Info => "IMPORTANT",
        CalloutTone::Warning => "WARNING",
        CalloutTone::Danger => "CAUTION",
    }
}

fn write_table(out: &mut String, rows: &[Vec<RichText>]) {
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    for (index, row) in rows.iter().enumerate() {
        out.push('|');
        for column in 0..width {
            let cell = row.get(column).map(cell_text).unwrap_or_default();
            out.push_str(&format!(" {} |", cell.replace('|', "\\|")));
        }
        if index == 0 {
            out.push_str("\n|");
            out.push_str(&" --- |".repeat(width));
        }
        if index + 1 < rows.len() {
            out.push('\n');
        }
    }
}

fn cell_text(cell: &RichText) -> String {
    cell.blocks
        .iter()
        .map(|block| render_markdown(&block.text, &block.marks))
        .collect::<Vec<_>>()
        .join(" ")
}
