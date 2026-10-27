//! The HTML preview and plain text of `.ods` sheets.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use quick_xml::escape::escape;

use super::sheet::{MAX_TEXT_CELLS, PREVIEW_COLS, PREVIEW_ROWS, Sheet};

impl Sheet {
    fn extent(&self) -> (u32, u32) {
        let (rows, _) = self.filled(PREVIEW_ROWS as usize * PREVIEW_COLS as usize);
        let max_row = rows.last().map_or(0, |(row, _)| *row);
        let max_col = rows
            .iter()
            .flat_map(|(_, cells)| cells.iter().map(|(col, _)| *col))
            .max()
            .unwrap_or(0);
        (max_col.min(PREVIEW_COLS), max_row.min(PREVIEW_ROWS))
    }

    pub(super) fn preview(&self, out: &mut String) {
        let (max_col, max_row) = self.extent();
        out.push_str("<table><caption>");
        out.push_str(&escape(self.name.as_str()));
        out.push_str("</caption>");
        for row in 1..=max_row {
            out.push_str("<tr>");
            for col in 1..=max_col {
                out.push_str("<td>");
                out.push_str(&escape(self.display(col, row).as_str()));
                out.push_str("</td>");
            }
            out.push_str("</tr>");
        }
        out.push_str("</table>\n");
    }

    pub(super) fn plain_text(&self, out: &mut String) {
        out.push_str("# ");
        out.push_str(&self.name);
        out.push('\n');
        let (rows, _) = self.filled(MAX_TEXT_CELLS);
        for (_, cells) in rows {
            out.push_str(
                &cells
                    .iter()
                    .map(|(_, c)| c.display.as_str())
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
            out.push('\n');
        }
    }
}
