//! The `model` guide topic: the `RichText` v2 model, worked examples for a
//! rich `documents_create`, the layout ops, and the model's JSON Schema
//! (the same one `/api/v1/documents/schema` serves).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::documents::model::model_view_schema;

const MODEL: &str = "\
A text document (docx, md, html, odt) is a RichText v2 model: {version: 2, kind: rich_text, \
blocks: [...], document?: {...}}. Read it with documents_read view: model; each block's id is \
the node id the edit ops address.
Block: {id, type, attrs?, text?, marks?} plus the type's fields. Types: paragraph; heading \
{level 1-6}; list_item {level 0-8, ordered, numbering?: <numbering id>, checked?: bool}; quote; \
code {language?}; callout {tone: info|success|warning|danger|note}; footnote {label}; table \
{rows: [[{blocks: [...]}]], props?: {columns: [{width pt}], header_rows, style, cells: \
[[{shade: RRGGBB, align, valign: top|center|bottom, colspan, rowspan}]]}}; image {blob: \
<blob_ref from documents_upload>, alt, width px, height px, caption, wrap: \
inline|square|top_bottom|behind|in_front}; divider; page_break; section_break {section}; toc \
{levels}. A cell covered by a span keeps its slot, empty.
attrs: {style: <style id>, align: left|center|right|justify, indent: {left, right, \
first_line (negative hangs)}, space_before, space_after, keep_with_next}; lengths in points.
marks: [{start, end, mark, ...}] over the block's text in characters: bold, italic, \
underline, strike, code, superscript, subscript, caps, link {href}, color {value: #RRGGBB}, \
highlight {value: yellow|green|... or #RRGGBB}, font {value: family}, size {value: points}, \
field {code: PAGE|NUMPAGES|DATE} (the marked text is the shown value), footnote {label}.
document: {styles: [{id, name, kind: paragraph|character|table, based_on, next, font, size, \
bold, italic, color, paragraph: attrs}], theme: {heading_font, body_font, palette: [6 \
RRGGBB]}, numbering: [{id, levels: [{format: bullet|decimal|lower_letter|upper_letter|\
lower_roman|upper_roman, text: \"%1.\", start, indent}]}], section: {size: {width, height}, \
orientation, margins: {top, right, bottom, left}, header, footer, first_page_header, \
first_page_footer, even_header, even_footer (each {blocks})}, front_matter (md)}.
Create: documents_create {template_id, name, content: {model: {...}} | {markdown: \"...\"}, \
ops?}. The template's theme, styles, numbering and headers and footers stay unless content \
names its own; content replaces the template's body, then ops apply. Example: content: \
{model: {blocks: [{type: heading, level: 1, text: \"Q3 review\"}, {type: paragraph, text: \
\"Revenue grew 12%.\", marks: [{start: 0, end: 7, mark: bold}]}, {type: callout, tone: \
warning, text: \"Draft: figures unaudited.\"}, {type: table, rows: [[{blocks: [{type: \
paragraph, text: \"Region\"}]}, {blocks: [{type: paragraph, text: \"Growth\"}]}], [{blocks: \
[{type: paragraph, text: \"EMEA\"}]}, {blocks: [{type: paragraph, text: \"9%\"}]}]], props: \
{header_rows: 1, columns: [{width: 200}, {width: 100}], cells: [[{shade: \"1F4E79\"}, {shade: \
\"1F4E79\"}]]}}, {type: image, blob: \"<blob_ref>\", alt: \"Chart\", width: 480, caption: \
\"Figure 1\"}], document: {section: {footer: {blocks: [{type: paragraph, text: \"Page 1\", \
attrs: {align: center}, marks: [{start: 5, end: 6, mark: field, code: PAGE}]}]}}}}}.
Layout ops (documents_edit; docx, and md where Markdown can hold the result): set_paragraph \
{node, expected_hash, attrs}; insert_block {after, block} (any block type, no id; images \
take a blob_ref); update_table {table, expected_hash, cells?: [{row, col, text?, props?}], \
widths?: [pt|null], header_rows?, style?}; set_section {section} (the last section); \
set_style {style} (adds or replaces by id); apply_theme {theme}. set_marks takes every mark \
above. Markdown refuses what it cannot hold (colour, fonts, sizes, fields, paragraph \
formatting, page and section breaks, styled or merged tables, sized images) with \
unsupported_op naming the block; nothing is dropped.";

pub(super) fn model_topic() -> String {
    let schema = serde_json::to_string(&model_view_schema()).unwrap_or_default();
    format!("{MODEL}\nJSON Schema (also GET /api/v1/documents/schema):\n{schema}")
}
