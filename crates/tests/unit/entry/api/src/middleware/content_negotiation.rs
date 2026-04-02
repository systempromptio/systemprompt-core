//! Unit tests for content negotiation middleware
//!
//! Tests cover:
//! - AcceptedMediaType content_type values
//! - AcceptedMediaType is_markdown checks
//! - AcceptedFormat wrapper behavior
//! - parse_accept_header with various Accept header values
//! - Quality factor parsing and priority

use systemprompt_api::services::middleware::{
    AcceptedFormat, AcceptedMediaType, parse_accept_header,
};

#[test]
fn json_content_type() {
    assert_eq!(AcceptedMediaType::Json.content_type(), "application/json");
}

#[test]
fn markdown_content_type() {
    assert_eq!(
        AcceptedMediaType::Markdown.content_type(),
        "text/markdown; charset=utf-8"
    );
}

#[test]
fn html_content_type() {
    assert_eq!(
        AcceptedMediaType::Html.content_type(),
        "text/html; charset=utf-8"
    );
}

#[test]
fn json_is_not_markdown() {
    assert!(!AcceptedMediaType::Json.is_markdown());
}

#[test]
fn markdown_is_markdown() {
    assert!(AcceptedMediaType::Markdown.is_markdown());
}

#[test]
fn html_is_not_markdown() {
    assert!(!AcceptedMediaType::Html.is_markdown());
}

#[test]
fn default_media_type_is_json() {
    let default = AcceptedMediaType::default();
    assert_eq!(default, AcceptedMediaType::Json);
}

#[test]
fn accepted_format_default_is_json() {
    let format = AcceptedFormat::default();
    assert_eq!(format.media_type(), AcceptedMediaType::Json);
}

#[test]
fn accepted_format_is_markdown_delegates() {
    let format = AcceptedFormat(AcceptedMediaType::Markdown);
    assert!(format.is_markdown());
}

#[test]
fn accepted_format_json_not_markdown() {
    let format = AcceptedFormat(AcceptedMediaType::Json);
    assert!(!format.is_markdown());
}

#[test]
fn parse_application_json() {
    let result = parse_accept_header("application/json");
    assert_eq!(result.media_type(), AcceptedMediaType::Json);
}

#[test]
fn parse_text_markdown() {
    let result = parse_accept_header("text/markdown");
    assert_eq!(result.media_type(), AcceptedMediaType::Markdown);
}

#[test]
fn parse_text_x_markdown() {
    let result = parse_accept_header("text/x-markdown");
    assert_eq!(result.media_type(), AcceptedMediaType::Markdown);
}

#[test]
fn parse_text_html() {
    let result = parse_accept_header("text/html");
    assert_eq!(result.media_type(), AcceptedMediaType::Html);
}

#[test]
fn parse_xhtml() {
    let result = parse_accept_header("application/xhtml+xml");
    assert_eq!(result.media_type(), AcceptedMediaType::Html);
}

#[test]
fn parse_wildcard_defaults_to_json() {
    let result = parse_accept_header("*/*");
    assert_eq!(result.media_type(), AcceptedMediaType::Json);
}

#[test]
fn parse_empty_string_defaults_to_json() {
    let result = parse_accept_header("");
    assert_eq!(result.media_type(), AcceptedMediaType::Json);
}

#[test]
fn parse_unknown_type_defaults_to_json() {
    let result = parse_accept_header("application/xml");
    assert_eq!(result.media_type(), AcceptedMediaType::Json);
}

#[test]
fn parse_quality_factor_higher_wins() {
    let result = parse_accept_header("text/html;q=0.5, text/markdown;q=0.9");
    assert_eq!(result.media_type(), AcceptedMediaType::Markdown);
}

#[test]
fn parse_quality_factor_default_is_one() {
    let result = parse_accept_header("text/markdown, text/html;q=0.5");
    assert_eq!(result.media_type(), AcceptedMediaType::Markdown);
}

#[test]
fn parse_multiple_types_first_highest_quality_wins() {
    let result = parse_accept_header("application/json;q=0.8, text/html;q=1.0");
    assert_eq!(result.media_type(), AcceptedMediaType::Html);
}

#[test]
fn parse_quality_zero_deprioritized() {
    let result = parse_accept_header("text/html;q=0, application/json");
    assert_eq!(result.media_type(), AcceptedMediaType::Json);
}

#[test]
fn parse_with_extra_whitespace() {
    let result = parse_accept_header("  text/markdown  ,  application/json;q=0.5  ");
    assert_eq!(result.media_type(), AcceptedMediaType::Markdown);
}

#[test]
fn parse_quality_clamped_above_one() {
    let result = parse_accept_header("text/html;q=2.0, application/json;q=0.9");
    assert_eq!(result.media_type(), AcceptedMediaType::Html);
}
