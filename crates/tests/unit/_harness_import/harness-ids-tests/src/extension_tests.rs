//! Extension ids: shape, parsing, serde and the compile-time macro.

use harness_ids::{ExtensionId, ExtensionLiteral, extension_id};

#[test]
fn well_formed_ids_parse() {
    for value in ["drive", "brain", "a", "sp-slack", "x2", "notifications-v2"] {
        assert_eq!(ExtensionId::parse(value).unwrap().as_str(), value);
    }
}

#[test]
fn malformed_ids_are_refused() {
    let too_long = "a".repeat(ExtensionId::MAX_LEN + 1);
    for value in [
        "",
        "Drive",
        "2fa",
        "-drive",
        "drive-",
        "dr--ive",
        "dr ive",
        "drive_ui",
        too_long.as_str(),
    ] {
        let error = ExtensionId::parse(value).unwrap_err();
        assert_eq!(error.kind, "extension");
        assert_eq!(error.value, value);
    }
}

#[test]
fn the_longest_id_is_accepted() {
    let longest = "a".repeat(ExtensionId::MAX_LEN);
    assert!(ExtensionId::parse(&longest).is_ok());
}

#[test]
fn the_macro_matches_parse() {
    const DRIVE: ExtensionId = extension_id!("drive");
    assert_eq!(DRIVE, ExtensionId::parse("drive").unwrap());
    assert_eq!(extension_id!("sp-slack").to_string(), "sp-slack");
}

#[test]
fn a_literal_is_checked_before_it_becomes_an_id() {
    assert!(ExtensionLiteral::new("Not-Valid").is_none());
    assert_eq!(
        ExtensionLiteral::new("valid").map(ExtensionLiteral::id),
        ExtensionId::parse("valid").ok()
    );
}

#[test]
fn serde_round_trips_and_validates() {
    let id: ExtensionId = serde_json::from_str("\"brain\"").unwrap();
    assert_eq!(serde_json::to_string(&id).unwrap(), "\"brain\"");
    assert!(serde_json::from_str::<ExtensionId>("\"Brain\"").is_err());
}

#[test]
fn ids_order_by_their_text() {
    let mut ids = [
        extension_id!("drive"),
        extension_id!("brain"),
        extension_id!("slack"),
    ];
    ids.sort();
    let names: Vec<_> = ids.iter().map(ExtensionId::as_str).collect();
    assert_eq!(names, ["brain", "drive", "slack"]);
}
