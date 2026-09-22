//! Schema pin for request-origin attribution.
//!
//! `ai_requests.client_kind`, `ai_requests.wire_protocol` and
//! `ai_requests.client_attestation` are CHECK constraints over the strings the
//! origin enums write, and `ai_request_client_evidence` mirrors two of them.
//! If any pair drifts, an insert fails in production; these tests fail first,
//! against both the base schema (fresh installs) and the migrations
//! (established databases).

use systemprompt_ai::models::{ClientAttestation, ClientKind, InboundWireProtocol, NativeMarker};

const BASE_SCHEMA: &str = include_str!("../../../../../../domain/ai/schema/ai_requests.sql");
const EVIDENCE_SCHEMA: &str =
    include_str!("../../../../../../domain/ai/schema/ai_request_client_evidence.sql");
const MIGRATION_026: &str =
    include_str!("../../../../../../domain/ai/schema/migrations/026_ai_requests_client_origin.sql");
const MIGRATION_027: &str = include_str!(
    "../../../../../../domain/ai/schema/migrations/027_ai_request_client_attestation.sql"
);
const MIGRATION_034: &str = include_str!(
    "../../../../../../domain/ai/schema/migrations/034_claude_metadata_json_marker.sql"
);

fn check_list<'a>(sql: &'a str, constraint: &str) -> &'a str {
    let start = sql
        .find(constraint)
        .unwrap_or_else(|| panic!("{constraint} missing"));
    let rest = &sql[start..];
    let open = rest.find("IN (").expect("IN (");
    let close = rest[open..].find("))").expect("))");
    &rest[open..open + close]
}

fn assert_list_is_exactly(list: &str, values: impl Iterator<Item = &'static str>, what: &str) {
    let mut count = 0;
    for value in values {
        assert!(
            list.contains(&format!("'{value}'")),
            "{value} missing from {what}"
        );
        count += 1;
    }
    assert_eq!(
        list.matches('\'').count() / 2,
        count,
        "{what} has extra values"
    );
}

#[test]
fn client_kind_strings_are_all_in_every_check_list() {
    for sql in [BASE_SCHEMA, MIGRATION_027] {
        assert_list_is_exactly(
            check_list(sql, "ai_requests_client_kind_check"),
            ClientKind::ALL.iter().map(|kind| kind.as_str()),
            "client_kind CHECK",
        );
    }
    for sql in [EVIDENCE_SCHEMA, MIGRATION_027] {
        assert_list_is_exactly(
            check_list(sql, "ai_request_client_evidence_attested_host_check"),
            ClientKind::ALL.iter().map(|kind| kind.as_str()),
            "attested_host CHECK",
        );
    }
    assert!(
        !check_list(MIGRATION_026, "ai_requests_client_kind_check").contains("'pi'"),
        "026 is history; pi arrived in 027"
    );
}

#[test]
fn wire_protocol_strings_are_all_in_both_check_lists() {
    for sql in [BASE_SCHEMA, MIGRATION_026] {
        assert_list_is_exactly(
            check_list(sql, "ai_requests_wire_protocol_check"),
            InboundWireProtocol::ALL.iter().map(|wire| wire.as_str()),
            "wire_protocol CHECK",
        );
    }
}

#[test]
fn client_attestation_strings_are_all_in_every_check_list() {
    for sql in [BASE_SCHEMA, MIGRATION_027] {
        assert_list_is_exactly(
            check_list(sql, "ai_requests_client_attestation_check"),
            ClientAttestation::ALL.iter().map(|tier| tier.as_str()),
            "client_attestation CHECK",
        );
    }
    for sql in [EVIDENCE_SCHEMA, MIGRATION_027] {
        assert_list_is_exactly(
            check_list(sql, "ai_request_client_evidence_kind_source_check"),
            ClientAttestation::ALL.iter().map(|tier| tier.as_str()),
            "kind_source CHECK",
        );
    }
    for sql in [EVIDENCE_SCHEMA, MIGRATION_034] {
        assert_list_is_exactly(
            check_list(sql, "ai_request_client_evidence_native_marker_check"),
            NativeMarker::ALL.iter().map(|marker| marker.as_str()),
            "native_marker CHECK",
        );
    }
    assert!(
        !check_list(
            MIGRATION_027,
            "ai_request_client_evidence_native_marker_check"
        )
        .contains("'claude-metadata-json'"),
        "027 is history; the marker vocabulary was re-cut in 034"
    );
}

#[test]
fn unknown_is_the_default_for_old_binaries_in_every_file() {
    for (sql, columns) in [
        (
            BASE_SCHEMA,
            &["client_kind", "wire_protocol", "client_attestation"][..],
        ),
        (MIGRATION_026, &["client_kind", "wire_protocol"][..]),
        (MIGRATION_027, &["client_attestation"][..]),
    ] {
        for column in columns {
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

#[test]
fn evidence_length_bounds_match_the_truncation() {
    for sql in [EVIDENCE_SCHEMA, MIGRATION_027] {
        for column in [
            "declared_client",
            "ua_product",
            "ua_version",
            "sdk_lang",
            "sdk_package_version",
            "sdk_runtime",
            "sdk_runtime_version",
            "sdk_os",
            "sdk_arch",
        ] {
            assert!(
                sql.contains(&format!("length({column}) <= 64")),
                "{column} bound"
            );
        }
    }
}
