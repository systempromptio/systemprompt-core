//! Rewrite of a pre-0.73 profile layer to the 0.73 shape.
//!
//! A 0.72 profile states the public URL several times (`api_server_url`,
//! `api_external_url`, `use_https`, `cors_allowed_origins`, `jwt_issuer`) and
//! the internal URLs once each. The 0.73 loader derives all of them from
//! `server.external_url` and the bind address, and refuses a profile that
//! states one. [`migrate_profile_layer`] replaces them with one
//! `server.external_url`, keeps the extra CORS origins as
//! `server.extra_cors_origins` and renames a top-level `evaluation:` to
//! `judge:`. The checks read `view`, the layer merged over the base it
//! loads with, because a value 0.72 resolved can come from either layer; the
//! edits touch only the keys this layer states. A layer that moves one
//! directory deeper
//! (`profiles/<name>.secrets.profile.yaml` to `profiles/<name>/profile.yaml`)
//! gains `paths.root: ".."` so its relative paths resolve where they did. It
//! edits the text line by line so comments and layout survive,
//! and it refuses whenever the derived value would differ from the stated
//! one: a JWT issuer that changed would stop every issued token verifying.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod checks;
mod lines;

use serde_yaml::Value;

use checks::{
    check_https, check_internal, check_issuer, extra_origins, is_reference, lookup, normalised,
    stated_external,
};
use lines::{
    find_block, first_stated, inline_mapping, insert_child, prune_if_empty, remove_block,
    replace_block, yaml_scalar,
};

#[derive(Debug, thiserror::Error)]
pub enum ProfileMigrationError {
    #[error("the profile is not valid YAML")]
    Parse(#[source] serde_yaml::Error),

    #[error(
        "`{key}` sits on an inline mapping; edit it to block style and run the migration again"
    )]
    InlineMapping { key: String },

    #[error(
        "neither server.api_external_url nor server.api_server_url is stated, so server.external_url cannot be chosen; state server.external_url"
    )]
    MissingExternalUrl,

    #[error(
        "`{first}` is '{first_value}' but `{second}` is '{second_value}'; make them agree, then run the migration again"
    )]
    Disagreement {
        first: &'static str,
        first_value: String,
        second: &'static str,
        second_value: String,
    },

    #[error(
        "security.jwt_issuer '{issuer}' differs from the external URL '{external}'. 0.73 issues tokens as the external URL, so every token issued as '{issuer}' would stop verifying; decide which URL is public and make both say it"
    )]
    IssuerChange { issuer: String, external: String },

    #[error(
        "server.api_internal_url '{stated}' differs from '{derived}', the URL 0.73 derives from server.host and server.port; change host/port or the URL so they agree"
    )]
    InternalUrlChange { stated: String, derived: String },

    #[error(
        "governance.authz.hook.url '{stated}' differs from '{derived}', the webhook URL 0.73 derives from server.host and server.port"
    )]
    HookUrlChange { stated: String, derived: String },

    #[error("the profile carries a top-level `{key}:` section; {reason}")]
    MovedSection {
        key: &'static str,
        reason: &'static str,
    },
}

const STATED_URL_FIELDS: &[&[&str]] = &[
    &["server", "api_server_url"],
    &["server", "api_external_url"],
    &["server", "use_https"],
    &["server", "cors_allowed_origins"],
    &["server", "api_internal_url"],
    &["security", "jwt_issuer"],
    &["governance", "authz", "hook", "url"],
];

const MOVED_SECTIONS: &[(&str, &str)] = &[
    (
        "providers",
        "the provider catalog lives in services/ai/; run `systemprompt admin config migrate` on the services tree",
    ),
    (
        "gateway",
        "gateway routes live in services/ai/gateway.yaml; run `systemprompt admin config migrate` on the services tree",
    ),
];

fn has_relative_path(root: &Value) -> bool {
    lookup(root, &["paths"])
        .and_then(Value::as_mapping)
        .is_some_and(|paths| {
            paths.iter().any(|(key, value)| {
                key.as_str() != Some("root")
                    && value.as_str().is_some_and(|p| {
                        !p.starts_with('/') && !p.starts_with('~') && !is_reference(p)
                    })
            })
        })
}

/// What [`migrate_profile_layer`] changed, for the command's report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileMigration {
    pub text: String,
    pub external_url: Option<String>,
    pub extra_cors_origins: Vec<String>,
    pub removed: Vec<String>,
    pub renamed_judge: bool,
    pub rooted_at_parent: bool,
}

fn tidy(url: String) -> String {
    if is_reference(&url) {
        url
    } else {
        normalised(&url)
    }
}

fn rewrite_urls(
    layer: &Value,
    view: &Value,
    lines: &mut Vec<String>,
    stated: &[&[&str]],
    migration: &mut ProfileMigration,
) -> Result<(), ProfileMigrationError> {
    let url_fields: [&[&str]; 2] = [
        &["server", "api_server_url"],
        &["server", "api_external_url"],
    ];
    let layer_states_url = url_fields.iter().any(|path| lookup(layer, path).is_some());
    let layer_external = layer_states_url
        .then(|| stated_external(layer))
        .transpose()?;
    let view_external = match stated_external(view) {
        Ok(url) => Some(url),
        Err(ProfileMigrationError::MissingExternalUrl) if !layer_states_url => None,
        Err(err) => return Err(err),
    };
    if let Some(external) = view_external.as_deref() {
        check_issuer(view, external)?;
        check_https(view, external)?;
    }
    check_internal(view)?;
    let extra = extra_origins(layer, view_external.as_deref().unwrap_or_default());

    if !extra.is_empty() {
        remove_block(lines, &["server", "extra_cors_origins"]);
        let mut block = vec!["extra_cors_origins:".to_owned()];
        block.extend(extra.iter().map(|o| format!("  - {}", yaml_scalar(o))));
        if !replace_block(lines, &["server", "cors_allowed_origins"], &block) {
            insert_child(lines, &["server"], &block);
        }
        migration.extra_cors_origins = extra;
    }
    if let Some(external) = layer_external
        && lookup(layer, &["server", "external_url"]).is_none()
    {
        let external = tidy(external);
        let line = [format!("external_url: {}", yaml_scalar(&external))];
        let replaced =
            first_stated(lines, &url_fields).is_some_and(|path| replace_block(lines, path, &line));
        if !replaced {
            insert_child(lines, &["server"], &line);
        }
        migration.external_url = Some(external);
    }
    for path in stated {
        if remove_block(lines, path) || lookup(layer, path).is_some() {
            migration.removed.push(path.join("."));
        }
    }
    let emptied: [&[&str]; 5] = [
        &["governance", "authz", "hook"],
        &["governance", "authz"],
        &["governance"],
        &["security"],
        &["server"],
    ];
    for path in emptied {
        prune_if_empty(lines, path);
    }
    Ok(())
}

pub fn migrate_profile_layer(
    text: &str,
    view: Option<&Value>,
    moved_down: bool,
) -> Result<Option<ProfileMigration>, ProfileMigrationError> {
    let root: Value = serde_yaml::from_str(text).map_err(ProfileMigrationError::Parse)?;
    for (key, reason) in MOVED_SECTIONS {
        if lookup(&root, &[key]).is_some() {
            return Err(ProfileMigrationError::MovedSection { key, reason });
        }
    }
    let stated: Vec<&[&str]> = STATED_URL_FIELDS
        .iter()
        .copied()
        .filter(|path| lookup(&root, path).is_some())
        .collect();
    let rename_judge = lookup(&root, &["evaluation"]).is_some();
    let root_paths =
        moved_down && lookup(&root, &["paths", "root"]).is_none() && has_relative_path(&root);
    if stated.is_empty() && !rename_judge && !root_paths {
        return Ok(None);
    }

    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    for path in [&["server"][..], &["security"], &["governance"], &["paths"]] {
        if inline_mapping(&lines, path) {
            return Err(ProfileMigrationError::InlineMapping {
                key: path.join("."),
            });
        }
    }

    let mut migration = ProfileMigration::default();
    if !stated.is_empty() {
        rewrite_urls(
            &root,
            view.unwrap_or(&root),
            &mut lines,
            &stated,
            &mut migration,
        )?;
    }
    if rename_judge
        && lookup(&root, &["judge"]).is_none()
        && let Some((index, _)) = find_block(&lines, &["evaluation"])
    {
        lines[index] = lines[index].replacen("evaluation:", "judge:", 1);
        migration.renamed_judge = true;
    }
    if root_paths && insert_child(&mut lines, &["paths"], &["root: \"..\"".to_owned()]) {
        migration.rooted_at_parent = true;
    }

    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    migration.text = out;
    Ok(Some(migration))
}
