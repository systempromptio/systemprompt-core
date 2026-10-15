//! Schema shaping, the entity tag and the plugin archive of the bridge
//! manifest routes.
//!
//! A schema-2 bridge reads a plugin-delivered skill's body from the plugin
//! it syncs, so its manifest names that file instead of inlining the body;
//! a bridge that advertises nothing keeps the inline schema. The entity tag
//! ignores the per-response stamp so an unchanged catalogue revalidates, and
//! an archive carries exactly the plugin's manifest file paths.

use std::io::Read;

use sha2::{Digest, Sha256};

use axum::http::{HeaderMap, header};
use chrono::{TimeZone, Utc};
use systemprompt_api::routes::gateway::bridge_manifest::shape;
use systemprompt_api::routes::gateway::bridge_plugin_archive::{archive_sha, pack};
use systemprompt_identifiers::PluginId;
use systemprompt_marketplace::{BundleFile, PluginBundle, skill_bundle_markdown};
use systemprompt_models::bridge::manifest::{
    MANIFEST_SCHEMA_HEADER, SignedManifest, SkillSource,
};
use systemprompt_models::bridge::manifest_version::ManifestVersion;

const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn manifest() -> SignedManifest {
    let mut manifest = unhashed_manifest();
    let markdown = skill_bundle_markdown(&manifest.skills[0]).expect("render SKILL.md");
    manifest.plugins[0].files[0].sha256 =
        hex::encode(Sha256::digest(markdown.as_bytes())).try_into().expect("digest");
    manifest
}

fn unhashed_manifest() -> SignedManifest {
    serde_json::from_value(serde_json::json!({
        "manifest_version": "2026-10-15T00:00:00Z-0000000000000001",
        "issued_at": "2026-10-15T00:00:00Z",
        "not_before": "2026-10-14T23:59:00Z",
        "user_id": "user-1",
        "tenant_id": null,
        "plugins": [{
            "id": "team-kit",
            "version": "1.0.0",
            "sha256": DIGEST,
            "files": [{ "path": "skills/write-docs/SKILL.md", "sha256": DIGEST, "size": 4 }],
        }],
        "skills": [
            {
                "id": "write_docs", "name": "write docs", "description": "", "file_path": "a.md",
                "sha256": DIGEST, "instructions": "body", "plugins": ["team-kit"],
            },
            {
                "id": "loose_skill", "name": "loose skill", "description": "", "file_path": "b.md",
                "sha256": DIGEST, "instructions": "inline body",
            },
        ],
        "managed_mcp_servers": [],
        "revocations": [],
    }))
    .expect("fixture manifest")
}

fn schema_headers(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(MANIFEST_SCHEMA_HEADER, value.parse().expect("schema header"));
    headers
}

#[test]
fn a_bridge_that_advertises_nothing_gets_the_inline_schema() {
    assert_eq!(shape::requested_schema(&HeaderMap::new()), 1);
    assert_eq!(shape::requested_schema(&schema_headers("garbage")), 1);
    assert_eq!(shape::requested_schema(&schema_headers("2")), 2);
    assert_eq!(shape::requested_schema(&schema_headers("99")), 2);
}

#[test]
fn the_inline_schema_keeps_every_body() {
    let mut manifest = manifest();
    shape::shape_for_schema(&mut manifest, 1);
    assert!(manifest.skills.iter().all(|skill| skill.instructions.is_some()));
    assert!(manifest.skills.iter().all(|skill| skill.source.is_none()));
}

#[test]
fn schema_two_names_the_plugin_file_and_keeps_bodies_no_plugin_carries() {
    let mut manifest = manifest();
    shape::shape_for_schema(&mut manifest, 2);

    let delivered = &manifest.skills[0];
    assert_eq!(delivered.instructions, None);
    assert_eq!(
        delivered.source,
        Some(SkillSource {
            plugin_id: PluginId::try_new("team-kit").expect("plugin id"),
            path: "skills/write-docs/SKILL.md".to_owned(),
        })
    );

    let loose = &manifest.skills[1];
    assert_eq!(loose.instructions.as_deref(), Some("inline body"));
    assert_eq!(loose.source, None);
}

#[test]
fn a_body_the_plugin_file_cannot_reproduce_stays_inline() {
    let mut drifted = manifest();
    drifted.skills[0].instructions = Some("body\n".to_owned());
    shape::shape_for_schema(&mut drifted, 2);
    assert_eq!(drifted.skills[0].instructions.as_deref(), Some("body\n"));
    assert_eq!(drifted.skills[0].source, None);

    let mut unhashed = unhashed_manifest();
    shape::shape_for_schema(&mut unhashed, 2);
    assert_eq!(unhashed.skills[0].instructions.as_deref(), Some("body"));
    assert_eq!(unhashed.skills[0].source, None);
}

#[test]
fn the_entity_tag_ignores_the_response_stamp_and_tracks_the_content() {
    let first = manifest();
    let mut restamped = manifest();
    restamped.issued_at = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();
    restamped.not_before = restamped.issued_at;
    restamped.manifest_version =
        ManifestVersion::try_new("2027-01-01T00:00:00Z-0000000000000002")
            .expect("manifest version");
    let tag = shape::entity_tag(&first).expect("tag");
    assert_eq!(tag, shape::entity_tag(&restamped).expect("tag"));
    assert!(tag.starts_with('"') && tag.ends_with('"'));

    let mut shaped = manifest();
    shape::shape_for_schema(&mut shaped, 2);
    assert_ne!(tag, shape::entity_tag(&shaped).expect("tag"));
}

#[test]
fn if_none_match_accepts_lists_weak_tags_and_the_wildcard() {
    let tag = "\"abc\"";
    let mut headers = HeaderMap::new();
    assert!(!shape::matches_if_none_match(&headers, tag));
    headers.insert(header::IF_NONE_MATCH, "\"zzz\", W/\"abc\"".parse().unwrap());
    assert!(shape::matches_if_none_match(&headers, tag));
    headers.insert(header::IF_NONE_MATCH, "*".parse().unwrap());
    assert!(shape::matches_if_none_match(&headers, tag));
    headers.insert(header::IF_NONE_MATCH, "\"zzz\"".parse().unwrap());
    assert!(!shape::matches_if_none_match(&headers, tag));
}

#[test]
fn only_a_full_digest_under_archive_is_an_archive_request() {
    assert_eq!(archive_sha(&format!("archive/{DIGEST}")), Some(DIGEST));
    assert_eq!(archive_sha("archive/abc"), None);
    assert_eq!(archive_sha("skills/write-docs/SKILL.md"), None);
    assert_eq!(archive_sha(&format!("archive/{DIGEST}/extra")), None);
}

#[test]
fn an_archive_holds_exactly_the_bundle_files_and_packs_deterministically() {
    let mut bundle = PluginBundle::new();
    bundle.insert(
        "skills/write-docs/SKILL.md".to_owned(),
        BundleFile {
            bytes: b"body".to_vec(),
            executable: false,
        },
    );
    bundle.insert(
        format!("scripts/{}/run.sh", "deep".repeat(40)),
        BundleFile {
            bytes: b"#!/bin/sh\n".to_vec(),
            executable: true,
        },
    );

    let packed = pack(&bundle).expect("pack");
    assert_eq!(packed, pack(&bundle).expect("pack again"));

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(packed.as_slice()));
    let mut seen = Vec::new();
    for entry in archive.entries().expect("entries") {
        let mut entry = entry.expect("entry");
        let path = entry.path().expect("path").to_string_lossy().into_owned();
        let mode = entry.header().mode().expect("mode");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("read");
        let file = bundle.get(&path).expect("every entry is a bundle path");
        assert_eq!(bytes, file.bytes);
        assert_eq!(mode, if file.executable { 0o755 } else { 0o644 });
        seen.push(path);
    }
    assert_eq!(seen, bundle.keys().cloned().collect::<Vec<_>>());
}
