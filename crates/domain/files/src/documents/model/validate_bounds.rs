//! The numeric and structural bounds of the v2 layout attributes, checked
//! by [`super::validate`]: lengths at most a 22-inch page (1584pt), image
//! sizes at most 4096px, at most 500 styles and 500 numbering definitions
//! of at most 9 levels, header rows within the table, and table spans that
//! stay inside the grid and never overlap another cell's span.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::document::DocumentSettings;
use super::layout::{ParaAttrs, Section, TableProps};
use super::rich_text::RichText;

pub(super) const MAX_POINTS: u32 = 1584;
pub(super) const MAX_PIXELS: u32 = 4096;
const MAX_STYLES: usize = 500;
const MAX_NUMBERING: usize = 500;
const MAX_LEVELS: usize = 9;
const MAX_COLUMNS: usize = 64;

pub(super) fn attrs_ok(attrs: &ParaAttrs) -> bool {
    let indent_ok = attrs.indent.as_ref().is_none_or(|indent| {
        indent.left <= MAX_POINTS
            && indent.right <= MAX_POINTS
            && indent.first_line.unsigned_abs() <= MAX_POINTS
    });
    indent_ok
        && attrs.space_before.is_none_or(|pt| pt <= MAX_POINTS)
        && attrs.space_after.is_none_or(|pt| pt <= MAX_POINTS)
        && attrs
            .style
            .as_deref()
            .is_none_or(|style| style.len() <= 128)
}

pub(super) fn image_ok(blob: &str, width: Option<u32>, height: Option<u32>) -> bool {
    let size_ok = [width, height]
        .into_iter()
        .flatten()
        .all(|px| (1..=MAX_PIXELS).contains(&px));
    size_ok && blob_ok(blob)
}

fn blob_ok(blob: &str) -> bool {
    let lower = blob.trim().to_ascii_lowercase();
    if let Some(target) = lower.strip_prefix("docx-media:") {
        let image = [
            ".png", ".jpg", ".jpeg", ".gif", ".bmp", ".tif", ".tiff", ".emf", ".wmf",
        ]
        .iter()
        .any(|ext| target.ends_with(ext));
        return target.starts_with("media/") && !target.contains("..") && image;
    }
    if lower.starts_with("https:") || lower.starts_with("http:") {
        return true;
    }
    lower
        .split(['/', '?', '#'])
        .next()
        .is_some_and(|head| !head.contains(':'))
}

pub(super) fn table_ok(rows: &[Vec<RichText>], props: &TableProps) -> bool {
    let columns = rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(props.columns.len());
    if columns > MAX_COLUMNS
        || props.cells.len() > rows.len()
        || usize::try_from(props.header_rows).is_ok_and(|h| h > rows.len())
        || props
            .columns
            .iter()
            .any(|column| column.width.is_some_and(|w| w > MAX_POINTS))
    {
        return false;
    }
    let mut taken = vec![vec![false; columns]; rows.len()];
    for (r, styles) in props.cells.iter().enumerate() {
        for (c, cell) in styles.iter().enumerate() {
            let (cols, rows_spanned) = (cell.colspan() as usize, cell.rowspan() as usize);
            if cols == 1 && rows_spanned == 1 {
                continue;
            }
            let fits = c.checked_add(cols).is_some_and(|end| end <= columns)
                && r.checked_add(rows_spanned)
                    .is_some_and(|end| end <= rows.len());
            if !fits {
                return false;
            }
            for row in taken.iter_mut().skip(r).take(rows_spanned) {
                for slot in row.iter_mut().skip(c).take(cols) {
                    if *slot {
                        return false;
                    }
                    *slot = true;
                }
            }
        }
    }
    true
}

pub(super) fn section_ok(section: &Section) -> bool {
    let size_ok = section.size.as_ref().is_none_or(|size| {
        (1..=MAX_POINTS).contains(&size.width) && (1..=MAX_POINTS).contains(&size.height)
    });
    let margins_ok = section.margins.as_ref().is_none_or(|m| {
        [m.top, m.right, m.bottom, m.left]
            .iter()
            .all(|pt| *pt <= MAX_POINTS)
    });
    size_ok && margins_ok
}

pub(super) fn settings_ok(settings: &DocumentSettings) -> bool {
    let styles_ok = settings.styles.len() <= MAX_STYLES
        && settings.styles.iter().all(|style| {
            !style.id.is_empty()
                && style.id.len() <= 128
                && attrs_ok(&style.paragraph)
                && style.size.as_deref().is_none_or(size_ok)
        });
    let numbering_ok = settings.numbering.len() <= MAX_NUMBERING
        && settings.numbering.iter().all(|def| {
            !def.id.is_empty()
                && def.id.len() <= 128
                && def.levels.len() <= MAX_LEVELS
                && def.levels.iter().all(|level| {
                    level.start <= 32_767 && level.indent.is_none_or(|i| i <= MAX_POINTS)
                })
        });
    let palette_ok = settings
        .theme
        .as_ref()
        .is_none_or(|theme| theme.palette.len() <= 6);
    styles_ok && numbering_ok && palette_ok && settings.section.as_ref().is_none_or(section_ok)
}

fn size_ok(size: &str) -> bool {
    size.parse::<f32>()
        .is_ok_and(|pt| (1.0..=1638.0).contains(&pt))
}
