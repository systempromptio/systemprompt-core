//! Schema shaping and conditional-request support for the signed manifest.
//!
//! A bridge that sends `x-bridge-manifest-schema: 2` reads a plugin-delivered
//! skill's body from the plugin it already syncs, so its manifest names that
//! file instead of inlining the body. A bridge that sends nothing gets the
//! inline schema every bridge since the floor parses.
//!
//! The bridge recovers the body as the text after the first `\n---\n\n` of
//! that `SKILL.md`, less one trailing newline, and checks it against the
//! skill digest. A body is dropped only when that recovery is proven exact:
//! the rendered `SKILL.md` hashes to the plugin file's manifest digest and
//! yields the instructions byte for byte. Anything else stays inline.
//!
//! The entity tag digests the manifest without its per-response stamp
//! (`manifest_version`, `issued_at`, `not_before`), so an unchanged catalogue
//! revalidates with an empty 304 instead of being re-signed and re-sent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use axum::http::HeaderMap;
use axum::http::header::IF_NONE_MATCH;
use sha2::{Digest, Sha256};
use systemprompt_identifiers::PluginId;
use systemprompt_marketplace::{skill_bundle_markdown, skill_bundle_path};
use systemprompt_models::bridge::manifest::{
    MANIFEST_INLINE_SCHEMA_VERSION, MANIFEST_SCHEMA_HEADER, MANIFEST_SCHEMA_VERSION,
    SignedManifest, SkillEntry, SkillSource,
};

use super::super::bridge_error::BridgeError;

const STAMP_FIELDS: [&str; 3] = ["manifest_version", "issued_at", "not_before"];

#[must_use]
pub fn requested_schema(headers: &HeaderMap) -> u32 {
    headers
        .get(MANIFEST_SCHEMA_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u32>().ok())
        .map_or(MANIFEST_INLINE_SCHEMA_VERSION, |schema| {
            schema.clamp(MANIFEST_INLINE_SCHEMA_VERSION, MANIFEST_SCHEMA_VERSION)
        })
}

pub fn shape_for_schema(manifest: &mut SignedManifest, schema: u32) {
    if schema < 2 {
        return;
    }
    let files: BTreeMap<&PluginId, BTreeMap<&str, &str>> = manifest
        .plugins
        .iter()
        .map(|plugin| {
            let paths = plugin
                .files
                .iter()
                .map(|file| (file.path.as_str(), file.sha256.as_str()))
                .collect();
            (&plugin.id, paths)
        })
        .collect();
    for skill in &mut manifest.skills {
        let path = skill_bundle_path(&skill.id);
        let Some(file_digest) = recoverable_digest(skill) else {
            continue;
        };
        let Some(owner) = skill.plugins.iter().find(|plugin| {
            files
                .get(plugin)
                .and_then(|paths| paths.get(path.as_str()))
                .is_some_and(|digest| *digest == file_digest)
        }) else {
            continue;
        };
        skill.source = Some(SkillSource {
            plugin_id: owner.clone(),
            path,
        });
        skill.instructions = None;
    }
}

// Why: the bridge verifies a recovered body against the skill digest, so a
// body is only withheld when the plugin file provably yields it unchanged.
fn recoverable_digest(skill: &SkillEntry) -> Option<String> {
    let instructions = skill.instructions.as_deref()?;
    let markdown = skill_bundle_markdown(skill).ok()?;
    let recovered = markdown
        .split_once("\n---\n\n")
        .and_then(|(_, body)| body.strip_suffix('\n'));
    (recovered == Some(instructions)).then(|| hex::encode(Sha256::digest(markdown.as_bytes())))
}

pub fn entity_tag(manifest: &SignedManifest) -> Result<String, BridgeError> {
    let mut value = serde_json::to_value(manifest)
        .map_err(|e| BridgeError::internal("manifest: entity tag serialisation failed", e))?;
    if let Some(fields) = value.as_object_mut() {
        for stamp in STAMP_FIELDS {
            fields.remove(stamp);
        }
    }
    let bytes = serde_json::to_vec(&value)
        .map_err(|e| BridgeError::internal("manifest: entity tag serialisation failed", e))?;
    Ok(format!("\"{}\"", hex::encode(Sha256::digest(&bytes))))
}

#[must_use]
pub fn matches_if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|candidate| candidate.trim().trim_start_matches("W/"))
        .any(|candidate| candidate == etag || candidate == "*")
}
