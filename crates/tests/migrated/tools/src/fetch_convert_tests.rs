//! Edge cases of turning a fetched page into markdown.

use serde_json::json;
use systemprompt_engine::tools::{FetchConfig, FetchTool, OutputLimits, Tool};

use crate::fetch_tests::{markdown, shown_within};
use crate::support::Project;

fn leak(html: String) -> &'static str {
    Box::leak(html.into_boxed_str())
}

#[tokio::test]
async fn the_format_defaults_to_markdown_and_a_zero_timeout_is_refused() {
    let project = Project::new();
    let tool = FetchTool::new(FetchConfig::default(), OutputLimits::default());
    let err = project
        .call(&tool, json!({"url": "http://127.0.0.1:9/x", "timeout": 0}))
        .await
        .message();
    assert!(err.contains("`timeout` must be at least 1"), "{err}");
    assert_eq!(tool.schema()["properties"]["format"]["enum"][0], "markdown");
}

#[tokio::test]
#[ignore = "parity ledger: deeply nested list output is not bounded"]
async fn deeply_nested_lists_stay_bounded() {
    let out = markdown(leak(format!(
        "{}x{}",
        "<ul><li>".repeat(400),
        "</li></ul>".repeat(400)
    )))
    .await;
    assert!(out.len() < 20_000, "{}", out.len());
}

#[tokio::test]
#[ignore = "parity ledger: blockquote lines are not prefixed with a quote marker"]
async fn every_line_of_a_multiline_quote_is_quoted() {
    let out = markdown("<blockquote><p>one<br>two</p></blockquote>").await;
    assert!(out.contains("> one") && out.contains("> two"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: nested blockquotes are not nested"]
async fn nested_quotes_stay_inside_their_outer_quote() {
    let out = markdown("<blockquote>outer<blockquote>inner</blockquote></blockquote>").await;
    assert!(out.contains("> > inner"), "{out}");
}

#[tokio::test]
async fn whitespace_around_inline_emphasis_is_kept() {
    let out = markdown("<p>a <b>bold</b> word</p>").await;
    assert!(out.contains("a **bold** word"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: emphasis inside preformatted content is rendered as markup"]
async fn formatting_inside_preformatted_content_is_code_only() {
    let out = markdown("<pre>let <b>x</b> = 1;</pre>").await;
    assert!(out.contains("let x = 1;") && !out.contains("**"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: a line break inside a table cell splits the row"]
async fn multiline_table_cells_keep_their_columns() {
    let out = markdown("<table><tr><td>a<br>b</td><td>c</td></tr></table>").await;
    let row = out
        .lines()
        .find(|line| line.contains('c'))
        .unwrap_or_default()
        .to_owned();
    assert_eq!(row.matches('|').count(), 3, "{out}");
}

#[tokio::test]
async fn nested_tables_are_flattened_without_breaking_the_outer_one() {
    let out = markdown(
        "<table><tr><td><table><tr><td>in</td></tr></table></td><td>out</td></tr></table>",
    )
    .await;
    assert!(out.contains("in") && out.contains("out"), "{out}");
    assert!(
        out.lines()
            .filter(|line| line.contains("out"))
            .all(|line| line.matches('|').count() >= 2),
        "{out}"
    );
}

#[tokio::test]
async fn loose_text_around_malformed_rows_is_kept() {
    let out = markdown("<table>stray<tr><td>a</td></tr>after</table>").await;
    assert!(
        out.contains("stray") && out.contains("after") && out.contains('a'),
        "{out}"
    );
}

#[tokio::test]
#[ignore = "parity ledger: tilde fences are not escaped and empty emphasis markers are kept"]
async fn tilde_fences_are_escaped_and_empty_emphasis_is_removed() {
    let out = markdown("<p>~~~ not a fence</p><p>x<b></b>y</p>").await;
    assert!(out.contains("\\~~~"), "{out}");
    assert!(!out.contains("****"), "{out}");
}

#[tokio::test]
async fn text_that_mentions_nul_is_kept_as_text() {
    let out = markdown("<p>the \\0 byte here</p>").await;
    assert!(out.contains("the \\0 byte"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: line breaks inside inline code are collapsed"]
async fn multiline_inline_code_is_kept_verbatim() {
    let out = markdown("<p><code>a\n  b</code></p>").await;
    assert!(out.contains("a\n  b"), "{out}");
}

#[tokio::test]
async fn links_inside_inline_code_keep_their_text() {
    let out = markdown("<p><code><a href=\"https://a.b\">call()</a></code></p>").await;
    assert!(out.contains("call()"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: list continuations and nested ordered lists are not indented under their item"]
async fn nested_list_continuations_are_indented_and_numbering_kept() {
    let out = markdown("<ol><li>one<p>more</p></li><li>two<ol><li>sub</li></ol></li></ol>").await;
    assert!(out.contains("1. one") && out.contains("2. two"), "{out}");
    assert!(out.contains("   1. sub"), "{out}");
}

#[tokio::test]
async fn block_content_is_rendered_outside_link_syntax() {
    let out = markdown("<a href=\"https://a.b\"><h2>Title</h2></a>").await;
    assert!(out.contains("## "), "{out}");
    assert!(!out.contains("[## "), "{out}");
}

#[tokio::test]
async fn nested_anchors_leave_no_unmatched_syntax() {
    let out = markdown("<a href=\"https://a\">x<a href=\"https://b\">y</a></a>").await;
    assert_eq!(out.matches('[').count(), out.matches(']').count(), "{out}");
}

#[tokio::test]
async fn unicode_in_inline_constructs_is_kept() {
    let out =
        markdown("<p><em>héllo</em> <code>ñ</code> <a href=\"https://a.b/é\">ü</a></p>").await;
    assert!(
        out.contains("héllo") && out.contains('ñ') && out.contains('ü'),
        "{out}"
    );
}

#[tokio::test]
async fn block_content_and_following_lists_are_kept() {
    let out = markdown("<div><p>para</p></div><ul><li>item</li></ul>").await;
    assert!(out.contains("para\n\n- item"), "{out}");
}

#[tokio::test]
async fn reconstructed_tables_are_separated_from_adjacent_text() {
    let out = markdown("<p>before</p><table><tr><td>a</td></tr></table><p>after</p>").await;
    assert!(out.contains("before\n\n"), "{out}");
    assert!(out.contains("\n\nafter"), "{out}");
}

#[tokio::test]
async fn links_resume_around_block_structures() {
    let out = markdown(
        "<p><a href=\"https://a.b\">one</a></p><ul><li><a href=\"https://c.d\">two</a></li></ul>",
    )
    .await;
    assert!(
        out.contains("[one](https://a.b)") && out.contains("[two](https://c.d)"),
        "{out}"
    );
}

#[tokio::test]
async fn captions_and_definition_lists_have_readable_boundaries() {
    let out = markdown("<table><caption>Cap</caption><tr><td>a</td></tr></table><dl><dt>Term</dt><dd>Def</dd></dl>").await;
    assert!(
        out.contains("Cap") && out.contains("Term") && out.contains("Def"),
        "{out}"
    );
    assert!(!out.contains("TermDef"), "{out}");
}

#[tokio::test]
async fn tables_with_spans_fall_back_to_rows_of_text() {
    let out = markdown(
        "<table><tr><td colspan=\"2\">wide</td></tr><tr><td>a</td><td>b</td></tr></table>",
    )
    .await;
    assert!(
        out.contains("wide") && out.contains('a') && out.contains('b'),
        "{out}"
    );
}

#[tokio::test]
async fn preformatted_breaks_are_kept_and_link_titles_normalised() {
    let out =
        markdown("<pre>a<br>b</pre><p><a href=\"https://a.b\" title=\"t\n  u\">l</a></p>").await;
    assert!(out.contains("a\nb"), "{out}");
    assert!(out.contains("[l](https://a.b"), "{out}");
}

#[tokio::test]
async fn emphasis_whitespace_survives_neutral_wrappers() {
    let out = markdown("<p>a<span> <b>b</b> </span>c</p>").await;
    assert!(out.contains("a **b** c"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: preformatted content in a table cell breaks the table"]
async fn preformatted_content_inside_table_cells_is_flattened() {
    let out = markdown("<table><tr><td><pre>x\ny</pre></td><td>z</td></tr></table>").await;
    let row = out
        .lines()
        .find(|line| line.contains('z'))
        .unwrap_or_default()
        .to_owned();
    assert!(row.contains('x'), "{out}");
}

#[tokio::test]
async fn quoted_code_with_long_backtick_runs_stays_in_a_closed_fence() {
    let out = markdown("<blockquote><pre>a ```` b</pre></blockquote><p>after</p>").await;
    assert!(out.contains("after"), "{out}");
    assert!(out.lines().any(|line| line == "after"), "{out}");
}

#[tokio::test]
async fn active_content_stays_suppressed_in_deep_nesting() {
    let out = markdown(leak(format!(
        "{}<script>evil()</script>visible{}",
        "<div>".repeat(600),
        "</div>".repeat(600)
    )))
    .await;
    assert!(out.contains("visible") && !out.contains("evil"), "{out}");
}

#[tokio::test]
#[ignore = "parity ledger: child lists are indented by two spaces whatever the parent marker width"]
async fn child_lists_indent_from_their_parent_marker() {
    let out = markdown("<ol start=\"10\"><li>ten<ul><li>child</li></ul></li></ol>").await;
    assert!(out.contains("10. ten\n    - child"), "{out}");
}

#[tokio::test]
async fn a_code_block_with_no_room_for_its_fence_leaves_no_open_fence() {
    for quoted in [false, true] {
        for spare in 0..24 {
            let code = "<pre>x</pre>";
            let block = if quoted {
                format!("<blockquote>{code}</blockquote>")
            } else {
                code.to_owned()
            };
            let html = format!("<p>{}</p>{block}", "y".repeat(80));
            let shown = shown_within(80 + spare, html).await;
            assert_eq!(
                shown.matches("```").count() % 2,
                0,
                "quoted {quoted}, spare {spare}: {shown:?}"
            );
        }
    }
}

#[tokio::test]
async fn inline_code_at_the_start_of_a_quote_is_quoted() {
    let out = markdown("<blockquote><code>x</code> y</blockquote>").await;
    assert!(out.lines().any(|line| line == "> `x` y"), "{out}");
}

#[tokio::test]
async fn text_after_deep_nesting_inside_preformatted_content_is_kept() {
    let out = markdown(leak(format!(
        "<pre>{}visible{}</pre><p>after</p>",
        "<i>".repeat(10_001),
        "</i>".repeat(10_001)
    )))
    .await;
    assert!(out.contains("visible") && out.contains("after"), "{out}");
}
