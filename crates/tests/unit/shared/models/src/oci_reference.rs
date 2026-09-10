use std::str::FromStr;

use systemprompt_models::profile::{OciReference, OciReferenceError};

const DIGEST: &str = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[test]
fn parses_registry_repository_and_tag() {
    let r = OciReference::from_str("ghcr.io/systempromptio/services:v1.2.3").expect("parse");
    assert_eq!(r.registry, "ghcr.io");
    assert_eq!(r.repository, "systempromptio/services");
    assert_eq!(r.tag.as_deref(), Some("v1.2.3"));
    assert_eq!(r.digest, None);
    assert!(!r.is_pinned());
}

#[test]
fn parses_digest_reference_as_pinned() {
    let raw = format!("ghcr.io/org/services@{DIGEST}");
    let r = OciReference::from_str(&raw).expect("parse");
    assert_eq!(r.tag, None);
    assert_eq!(r.digest.as_deref(), Some(DIGEST));
    assert!(r.is_pinned());
    assert_eq!(r.to_string(), raw);
}

#[test]
fn parses_tag_and_digest_together() {
    let raw = format!("ghcr.io/org/services:v1@{DIGEST}");
    let r = OciReference::from_str(&raw).expect("parse");
    assert_eq!(r.tag.as_deref(), Some("v1"));
    assert_eq!(r.digest.as_deref(), Some(DIGEST));
    assert_eq!(r.to_string(), raw);
}

#[test]
fn registry_may_carry_a_port_without_being_read_as_a_tag() {
    let r = OciReference::from_str("registry.internal:5000/team/services:latest").expect("parse");
    assert_eq!(r.registry, "registry.internal:5000");
    assert_eq!(r.repository, "team/services");
    assert_eq!(r.tag.as_deref(), Some("latest"));
}

#[test]
fn port_only_registry_without_tag_keeps_the_port() {
    let r = OciReference::from_str("localhost:5000/services").expect("parse");
    assert_eq!(r.registry, "localhost:5000");
    assert_eq!(r.repository, "services");
    assert_eq!(r.tag, None);
}

#[test]
fn oci_scheme_prefix_is_accepted() {
    let r = OciReference::from_str("oci://ghcr.io/org/services:v1").expect("parse");
    assert_eq!(r.registry, "ghcr.io");
}

#[test]
fn unqualified_reference_is_rejected() {
    assert!(matches!(
        OciReference::from_str("alpine:3"),
        Err(OciReferenceError::MissingRegistry(_))
    ));
    assert!(matches!(
        OciReference::from_str("org/services:v1"),
        Err(OciReferenceError::MissingRegistry(_))
    ));
}

#[test]
fn empty_reference_is_rejected() {
    assert_eq!(OciReference::from_str(""), Err(OciReferenceError::Empty));
}

#[test]
fn short_or_uppercase_digest_is_rejected() {
    assert!(matches!(
        OciReference::from_str("ghcr.io/org/s@sha256:abc"),
        Err(OciReferenceError::InvalidDigest(_))
    ));
    assert!(matches!(
        OciReference::from_str(&format!("ghcr.io/org/s@{}", DIGEST.to_uppercase())),
        Err(OciReferenceError::InvalidDigest(_))
    ));
    assert!(matches!(
        OciReference::from_str("ghcr.io/org/s@md5:abc"),
        Err(OciReferenceError::InvalidDigest(_))
    ));
}

#[test]
fn uppercase_repository_is_rejected() {
    assert!(matches!(
        OciReference::from_str("ghcr.io/Org/Services:v1"),
        Err(OciReferenceError::InvalidRepository(_))
    ));
}

#[test]
fn empty_repository_segment_is_rejected() {
    assert!(matches!(
        OciReference::from_str("ghcr.io/org//services"),
        Err(OciReferenceError::InvalidRepository(_))
    ));
    assert!(matches!(
        OciReference::from_str("ghcr.io/"),
        Err(OciReferenceError::EmptyRepository(_))
    ));
}

#[test]
fn tag_starting_with_a_separator_is_rejected() {
    assert!(matches!(
        OciReference::from_str("ghcr.io/org/services:-v1"),
        Err(OciReferenceError::InvalidTag(_))
    ));
}

#[test]
fn display_round_trips_a_tagged_reference() {
    let raw = "ghcr.io/org/services:v1.0.0";
    assert_eq!(OciReference::from_str(raw).expect("parse").to_string(), raw);
}
