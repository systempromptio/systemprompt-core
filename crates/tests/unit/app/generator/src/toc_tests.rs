//! Unit tests for the table-of-contents generator (`generate_toc`).
//!
//! Exercises heading extraction (first-h1 skip, level filtering, code spans),
//! slug generation and deduplication, nested/sibling/dedent list nesting,
//! HTML escaping, and heading-id injection into pre-rendered HTML.

use systemprompt_generator::generate_toc;

#[test]
fn no_headings_returns_empty_toc_and_unchanged_html() {
    let rendered = "<p>Just a paragraph</p>";
    let result = generate_toc("Just a paragraph with no headings", rendered);
    assert!(result.toc_html.is_empty());
    assert_eq!(result.content_html, rendered);
}

#[test]
fn lone_title_h1_is_skipped() {
    let md = "# Page Title\n\nsome body";
    let result = generate_toc(md, "<h1>Page Title</h1><p>some body</p>");
    assert!(result.toc_html.is_empty());
}

#[test]
fn h2_headings_produce_toc_entries() {
    let md = "# Title\n\n## First Section\n\n## Second Section";
    let result = generate_toc(md, "<h2>First Section</h2><h2>Second Section</h2>");
    assert!(result.toc_html.contains("<ul class=\"toc-list\">"));
    assert!(result.toc_html.contains("href=\"#first-section\""));
    assert!(result.toc_html.contains("href=\"#second-section\""));
    assert!(result.toc_html.contains(">First Section</a>"));
}

#[test]
fn heading_ids_injected_into_rendered_html() {
    let md = "# Title\n\n## Alpha Section";
    let rendered = "<h2>Alpha Section</h2>";
    let result = generate_toc(md, rendered);
    assert!(result.content_html.contains("id=\"alpha-section\""));
    assert!(
        result
            .content_html
            .contains("<h2 id=\"alpha-section\">Alpha Section</h2>")
    );
}

#[test]
fn existing_id_attribute_is_preserved() {
    let md = "# Title\n\n## Alpha Section";
    let rendered = "<h2 id=\"custom\">Alpha Section</h2>";
    let result = generate_toc(md, rendered);
    assert!(result.content_html.contains("id=\"custom\""));
    assert!(!result.content_html.contains("id=\"alpha-section\""));
}

#[test]
fn duplicate_heading_text_gets_deduplicated_slugs() {
    let md = "# Title\n\n## Repeat\n\n## Repeat";
    let result = generate_toc(md, "<h2>Repeat</h2><h2>Repeat</h2>");
    assert!(result.toc_html.contains("href=\"#repeat\""));
    assert!(result.toc_html.contains("href=\"#repeat-1\""));
}

#[test]
fn nested_headings_open_nested_list() {
    let md = "# Title\n\n## Parent\n\n### Child";
    let result = generate_toc(md, "<h2>Parent</h2><h3>Child</h3>");
    assert!(result.toc_html.contains("toc-nested"));
    assert!(result.toc_html.contains("toc-level-2"));
    assert!(result.toc_html.contains("toc-level-3"));
}

#[test]
fn dedent_closes_nested_list() {
    let md = "# Title\n\n## Parent\n\n### Child\n\n## Sibling";
    let result = generate_toc(md, "<h2>Parent</h2><h3>Child</h3><h2>Sibling</h2>");
    assert!(result.toc_html.contains("href=\"#parent\""));
    assert!(result.toc_html.contains("href=\"#child\""));
    assert!(result.toc_html.contains("href=\"#sibling\""));
    let opens = result.toc_html.matches("<ul").count();
    let closes = result.toc_html.matches("</ul>").count();
    assert_eq!(opens, closes, "balanced ul tags:\n{}", result.toc_html);
}

#[test]
fn code_span_in_heading_is_extracted_as_text() {
    let md = "# Title\n\n## Use `cargo build` now";
    let result = generate_toc(md, "<h2>Use <code>cargo build</code> now</h2>");
    assert!(result.toc_html.contains("cargo build"));
    assert!(result.toc_html.contains("href=\"#use-cargo-build-now\""));
}

#[test]
fn special_characters_are_html_escaped_in_toc() {
    let md = "# Title\n\n## A & B \"quote\"";
    let result = generate_toc(md, "<h2>heading</h2>");
    assert!(result.toc_html.contains("&amp;"));
    assert!(result.toc_html.contains("&quot;"));
    assert!(!result.toc_html.contains(" & "));
}

#[test]
fn empty_heading_text_is_skipped() {
    let md = "# Title\n\n##\n\n## Real Heading";
    let result = generate_toc(md, "<h2>Real Heading</h2>");
    assert!(result.toc_html.contains("href=\"#real-heading\""));
}

#[test]
fn multiple_h1_only_first_is_skipped() {
    let md = "# First Title\n\n# Second Heading\n\n## Sub";
    let result = generate_toc(md, "<h1>Second Heading</h1><h2>Sub</h2>");
    assert!(!result.toc_html.contains("First Title"));
}

#[test]
fn deep_nesting_across_multiple_levels() {
    let md = "# Title\n\n## L2\n\n### L3\n\n#### L4\n\n## Back2";
    let result = generate_toc(md, "<h2>L2</h2><h3>L3</h3><h4>L4</h4><h2>Back2</h2>");
    assert!(result.toc_html.contains("toc-level-4"));
    let opens = result.toc_html.matches("<ul").count();
    let closes = result.toc_html.matches("</ul>").count();
    assert_eq!(opens, closes);
}
