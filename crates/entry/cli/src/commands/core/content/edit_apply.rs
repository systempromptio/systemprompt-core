use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::Path;
use systemprompt_content::{CategoryIdUpdate, ContentRepository};
use systemprompt_identifiers::CategoryId;

use super::edit::EditArgs;

const VALID_KINDS: &[&str] = &["article", "paper", "guide", "tutorial"];

pub(crate) struct ContentEditState {
    pub title: String,
    pub description: String,
    pub body: String,
    pub keywords: String,
    pub image: Option<String>,
    pub category_id: CategoryIdUpdate,
    pub public_value: Option<bool>,
    pub kind_value: Option<String>,
}

pub(crate) fn apply_visibility_flags(
    args: &EditArgs,
    state: &mut ContentEditState,
    changes: &mut Vec<String>,
) {
    if args.public {
        state.public_value = Some(true);
        changes.push("public: true".to_owned());
    }
    if args.private {
        state.public_value = Some(false);
        changes.push("public: false".to_owned());
    }
}

pub(crate) fn apply_body_flags(
    args: &EditArgs,
    state: &mut ContentEditState,
    changes: &mut Vec<String>,
) -> Result<()> {
    if let Some(b) = &args.body {
        state.body.clone_from(b);
        changes.push("body: updated".to_owned());
    }
    if let Some(file) = &args.body_file {
        let path = Path::new(file);
        state.body = fs::read_to_string(path)
            .with_context(|| format!("Failed to read body file: {}", path.display()))?;
        changes.push("body: updated from file".to_owned());
    }
    Ok(())
}

pub(crate) async fn apply_set_value_flags(
    args: &EditArgs,
    state: &mut ContentEditState,
    changes: &mut Vec<String>,
    repo: &ContentRepository,
) -> Result<()> {
    for set_value in &args.set_values {
        let parts: Vec<&str> = set_value.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(anyhow!(
                "Invalid --set format: '{}'. Expected key=value",
                set_value
            ));
        }
        let key = parts[0].trim();
        let value = parts[1].trim();
        apply_set_field(key, value, state, changes, repo).await?;
    }
    Ok(())
}

async fn apply_set_field(
    key: &str,
    value: &str,
    state: &mut ContentEditState,
    changes: &mut Vec<String>,
    repo: &ContentRepository,
) -> Result<()> {
    match key {
        "title" => {
            state.title = value.to_owned();
            changes.push(format!("title: {}", value));
        },
        "description" => {
            state.description = value.to_owned();
            changes.push(format!("description: {}", value));
        },
        "keywords" => {
            state.keywords = value.to_owned();
            changes.push(format!("keywords: {}", value));
        },
        "image" => apply_image_field(value, state, changes),
        "category_id" | "category" => apply_category_field(value, state, changes, repo).await?,
        "kind" => apply_kind_field(value, state, changes)?,
        "public" => apply_public_field(value, state, changes)?,
        _ => {
            return Err(anyhow!(
                "Unknown field: '{}'. Supported fields: title, description, keywords, image, \
                 category_id, kind, public",
                key
            ));
        },
    }
    Ok(())
}

fn apply_image_field(value: &str, state: &mut ContentEditState, changes: &mut Vec<String>) {
    if value.eq_ignore_ascii_case("none") || value.is_empty() {
        state.image = None;
        changes.push("image: cleared".to_owned());
    } else {
        state.image = Some(value.to_owned());
        changes.push(format!("image: {}", value));
    }
}

async fn apply_category_field(
    value: &str,
    state: &mut ContentEditState,
    changes: &mut Vec<String>,
    repo: &ContentRepository,
) -> Result<()> {
    if value.eq_ignore_ascii_case("none") || value.is_empty() {
        state.category_id = CategoryIdUpdate::Clear;
        changes.push("category_id: cleared".to_owned());
        return Ok(());
    }
    let cat_id = CategoryId::new(value.to_owned());
    if !repo.category_exists(&cat_id).await? {
        return Err(anyhow!(
            "Category '{}' not found. Please use an existing category ID.",
            value
        ));
    }
    state.category_id = CategoryIdUpdate::Set(cat_id);
    changes.push(format!("category_id: {}", value));
    Ok(())
}

fn apply_kind_field(
    value: &str,
    state: &mut ContentEditState,
    changes: &mut Vec<String>,
) -> Result<()> {
    if !VALID_KINDS.contains(&value) {
        return Err(anyhow!(
            "Invalid kind '{}'. Must be one of: {}",
            value,
            VALID_KINDS.join(", ")
        ));
    }
    state.kind_value = Some(value.to_owned());
    changes.push(format!("kind: {}", value));
    Ok(())
}

fn apply_public_field(
    value: &str,
    state: &mut ContentEditState,
    changes: &mut Vec<String>,
) -> Result<()> {
    let p = value.parse::<bool>().map_err(|_e| {
        anyhow!(
            "Invalid boolean value for public: '{}'. Use true or false",
            value
        )
    })?;
    state.public_value = Some(p);
    changes.push(format!("public: {}", p));
    Ok(())
}
