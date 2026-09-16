//! Schema pin for request-origin attribution.
//!
//! `ai_requests.client_kind` and `ai_requests.wire_protocol` are CHECK
//! constraints over the strings `ClientKind::as_str` and
//! `InboundWireProtocol::as_str` write. If the two ever drift, an insert fails
//! in production; this test fails first, against both the base schema (fresh
//! installs) and migration 026 (established databases).

use systemprompt_ai::models::{ClientKind, InboundWireProtocol};

const BASE_SCHEMA: &str = include_str!("../../../../../domain/ai/schema/ai_requests.sql");
const MIGRATION: &str =
    include_str!("../../../../../domain/ai/schema/migrations/026_ai_requests_client_origin.sql");

fn check_list<'a>(sql: &'a str, constraint: &str) -> &'a str {
    let start = sql
        .find(constraint)
        .unwrap_or_else(|| panic!("{constraint} missing"));
    let rest = &sql[start..];
    let open = rest.find("IN (").expect("IN (");
    let close = rest[open..].find("))").expect("))");
    &rest[open..open + close]
}

#[test]
fn client_kind_strings_are_all_in_both_check_lists() {
    for sql in [BASE_SCHEMA, MIGRATION] {
        let list = check_list(sql, "ai_requests_client_kind_check");
        for kind in ClientKind::ALL {
            assert!(
                list.contains(&format!("'{}'", kind.as_str())),
                "{} missing from client_kind CHECK",
                kind.as_str()
            );
        }
        assert_eq!(list.matches('\'').count() / 2, ClientKind::ALL.len());
    }
}

#[test]
fn wire_protocol_strings_are_all_in_both_check_lists() {
    for sql in [BASE_SCHEMA, MIGRATION] {
        let list = check_list(sql, "ai_requests_wire_protocol_check");
        for wire in InboundWireProtocol::ALL {
            assert!(
                list.contains(&format!("'{}'", wire.as_str())),
                "{} missing from wire_protocol CHECK",
                wire.as_str()
            );
        }
        assert_eq!(
            list.matches('\'').count() / 2,
            InboundWireProtocol::ALL.len()
        );
    }
}

#[test]
fn unknown_is_the_default_for_old_binaries_in_both_files() {
    for sql in [BASE_SCHEMA, MIGRATION] {
        for column in ["client_kind", "wire_protocol"] {
            let declared = sql
                .lines()
                .find(|line| line.contains(column) && line.contains("NOT NULL DEFAULT"))
                .unwrap_or_else(|| panic!("{column} has no NOT NULL DEFAULT"));
            assert!(
                declared.contains(&format!("DEFAULT '{}'", ClientKind::Unknown.as_str())),
                "{column}: {declared}"
            );
        }
    }
}
