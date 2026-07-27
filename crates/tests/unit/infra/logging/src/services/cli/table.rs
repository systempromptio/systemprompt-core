//! Unit tests for cli::table rendering helpers.

use std::time::Duration;

use systemprompt_logging::services::cli::table::{
    ServiceTableEntry, render_service_table, render_service_table_into, render_startup_complete,
    render_table, truncate_to_width,
};
use systemprompt_logging::services::cli::theme::ServiceStatus;

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for next in chars.by_ref() {
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn render_service_lines(title: &str, services: &[ServiceTableEntry]) -> Vec<String> {
    let mut buf: Vec<u8> = Vec::new();
    render_service_table_into(&mut buf, title, services).expect("render into Vec cannot fail");
    String::from_utf8(buf)
        .expect("renderer emits utf-8")
        .lines()
        .map(strip_ansi)
        .filter(|line| !line.is_empty())
        .collect()
}

#[test]
fn truncate_to_width_returns_input_when_short_enough() {
    assert_eq!(truncate_to_width("hello", 10), "hello");
    assert_eq!(truncate_to_width("hello", 5), "hello");
}

#[test]
fn truncate_to_width_truncates_and_appends_ellipsis() {
    let out = truncate_to_width("abcdefghij", 6);
    assert_eq!(out, "abc...");
    assert!(out.chars().count() <= 6);
}

#[test]
fn truncate_to_width_zero_width_yields_ellipsis_only() {
    let out = truncate_to_width("abcdef", 3);
    assert_eq!(out, "...");
}

#[test]
fn truncate_to_width_handles_unicode() {
    let s = "日本語テストひらがな";
    let out = truncate_to_width(s, 5);
    assert!(out.ends_with("..."));
}

#[test]
fn render_table_empty_rows_no_panic() {
    render_table(&["a", "b"], &[]);
}

#[test]
fn render_table_single_row() {
    render_table(
        &["Name", "Type"],
        &[vec!["alpha".to_owned(), "service".to_owned()]],
    );
}

#[test]
fn render_table_multiple_rows_unequal_widths() {
    let rows = vec![
        vec!["x".to_owned(), "y-long-cell".to_owned()],
        vec!["very-long-name".to_owned(), "y".to_owned()],
    ];
    render_table(&["Col1", "Col2"], &rows);
}

#[test]
fn render_table_handles_extra_cells() {
    let rows = vec![vec!["a".to_owned(), "b".to_owned(), "extra".to_owned()]];
    render_table(&["X", "Y"], &rows);
}

#[test]
fn service_table_entry_builder() {
    let e = ServiceTableEntry::new("svc", "type", Some(1234), ServiceStatus::Running);
    assert_eq!(e.name, "svc");
    assert_eq!(e.service_type, "type");
    assert_eq!(e.port, Some(1234));
    assert!(matches!(e.status, ServiceStatus::Running));
}

#[test]
fn render_service_table_empty_no_panic() {
    render_service_table("Title", &[]);
}

#[test]
fn render_service_table_all_status_variants() {
    let services = vec![
        ServiceTableEntry::new("r", "t", Some(80), ServiceStatus::Running),
        ServiceTableEntry::new("s", "t", Some(81), ServiceStatus::Starting),
        ServiceTableEntry::new("o", "t", None, ServiceStatus::Stopped),
        ServiceTableEntry::new("f", "t", Some(83), ServiceStatus::Failed),
        ServiceTableEntry::new("u", "t", Some(84), ServiceStatus::Unknown),
    ];
    render_service_table("All Services", &services);
}

#[test]
fn render_service_table_every_line_is_the_same_width() {
    let services = vec![
        ServiceTableEntry::new("api", "http", Some(8080), ServiceStatus::Running),
        ServiceTableEntry::new(
            "a-considerably-longer-name",
            "subprocess",
            None,
            ServiceStatus::Stopped,
        ),
    ];

    let lines = render_service_lines("Services", &services);
    assert_eq!(
        lines.len(),
        8,
        "top border, title, rule, header, rule, 2 rows, bottom border"
    );

    let widths: Vec<usize> = lines.iter().map(|l| l.chars().count()).collect();
    assert!(
        widths.windows(2).all(|w| w[0] == w[1]),
        "ragged table frame: {widths:?}\n{}",
        lines.join("\n")
    );
}

#[test]
fn render_service_table_rows_are_framed_by_box_glyphs() {
    let services = vec![ServiceTableEntry::new(
        "api",
        "http",
        Some(8080),
        ServiceStatus::Running,
    )];

    let lines = render_service_lines("Services", &services);
    for line in &lines {
        let first = line.chars().next().expect("non-empty line");
        let last = line.chars().next_back().expect("non-empty line");
        assert!(
            "\u{250c}\u{2502}\u{251c}\u{2514}".contains(first),
            "line does not start on the frame: {line}"
        );
        assert!(
            "\u{2510}\u{2502}\u{2524}\u{2518}".contains(last),
            "line does not end on the frame: {line}"
        );
    }
}

#[test]
fn render_service_table_status_column_is_padded_not_collapsed() {
    let services = vec![
        ServiceTableEntry::new("a", "t", Some(80), ServiceStatus::Running),
        ServiceTableEntry::new("b", "t", Some(81), ServiceStatus::Starting),
    ];

    let lines = render_service_lines("Services", &services);
    let running = &lines[5];
    let starting = &lines[6];
    assert_eq!(
        running.chars().count(),
        starting.chars().count(),
        "status text of differing length must pad to the same column width:\n{running}\n{starting}"
    );
}

#[test]
fn render_startup_complete_smoke() {
    render_startup_complete(Duration::from_millis(1234), "http://localhost:8080");
    render_startup_complete(Duration::from_secs(0), "");
}
