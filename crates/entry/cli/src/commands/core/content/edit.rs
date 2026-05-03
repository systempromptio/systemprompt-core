use super::types::UpdateOutput;
use crate::cli_settings::CliConfig;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use anyhow::{Context, Result, anyhow};
use clap::Args;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use std::fs;
use std::path::Path;
use systemprompt_content::ContentRepository;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;

const VALID_KINDS: &[&str] = &["article", "paper", "guide", "tutorial"];

#[derive(Debug, Args)]
pub struct EditArgs {
    #[arg(help = "Content ID or slug to edit")]
    pub identifier: Option<String>,

    #[arg(long, help = "Source ID (required when using slug)")]
    pub source: Option<String>,

    #[arg(long = "set", value_name = "KEY=VALUE", help = "Set a field value")]
    pub set_values: Vec<String>,

    #[arg(long, help = "Make content public", conflicts_with = "private")]
    pub public: bool,

    #[arg(long, help = "Make content private", conflicts_with = "public")]
    pub private: bool,

    #[arg(long, help = "Body content")]
    pub body: Option<String>,

    #[arg(long, help = "File containing body content")]
    pub body_file: Option<String>,
}

pub async fn execute(args: EditArgs, config: &CliConfig) -> Result<CommandResult<UpdateOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub async fn execute_with_pool(
    args: EditArgs,
    pool: &DbPool,
    config: &CliConfig,
) -> Result<CommandResult<UpdateOutput>> {
    let repo = ContentRepository::new(pool)?;

    let identifier = resolve_required(args.identifier.clone(), "identifier", config, || {
        prompt_content_selection(&repo, args.source.as_deref(), config)
    })?;

    let content = if identifier.starts_with("content_")
        || identifier.contains('-') && identifier.len() > 30
    {
        let id = ContentId::new(identifier.clone());
        repo.get_by_id(&id)
            .await?
            .ok_or_else(|| anyhow!("Content not found: {}", identifier))?
    } else {
        let source_id = args
            .source
            .as_ref()
            .ok_or_else(|| anyhow!("Source ID required when using slug"))?;
        let source = SourceId::new(source_id.clone());
        repo.get_by_source_and_slug(&source, &identifier)
            .await?
            .ok_or_else(|| anyhow!("Content not found: {} in source {}", identifier, source_id))?
    };

    let mut changes = Vec::new();
    let mut state = ContentEditState {
        title: content.title.clone(),
        description: content.description.clone(),
        body: content.body.clone(),
        keywords: content.keywords.clone(),
        image: content.image.clone(),
        category_id: None,
        public_value: None,
        kind_value: None,
    };
    apply_visibility_flags(&args, &mut state, &mut changes);
    apply_body_flags(&args, &mut state, &mut changes)?;
    apply_set_value_flags(&args, &mut state, &mut changes, &repo).await?;

    if changes.is_empty() {
        return Err(anyhow!(
            "No changes specified. Use --set, --public, --private, --body, or --body-file"
        ));
    }

    CliService::info(&format!("Updating content '{}'...", content.slug));

    let params = systemprompt_content::UpdateContentParams::new(
        content.id.clone(),
        state.title,
        state.description,
        state.body,
    )
    .with_keywords(state.keywords)
    .with_image(state.image)
    .with_version_hash(content.version_hash.clone())
    .with_category_id(state.category_id)
    .with_public(state.public_value)
    .with_kind(state.kind_value);

    repo.update(&params).await?;

    CliService::success(&format!("Content '{}' updated successfully", content.slug));

    let output = UpdateOutput {
        content_id: content.id,
        slug: content.slug,
        updated_fields: changes,
        success: true,
    };

    Ok(CommandResult::text(output).with_title("Content Updated"))
}

struct ContentEditState {
    title: String,
    description: String,
    body: String,
    keywords: String,
    image: Option<String>,
    category_id: Option<Option<CategoryId>>,
    public_value: Option<bool>,
    kind_value: Option<String>,
}

fn apply_visibility_flags(args: &EditArgs, state: &mut ContentEditState, changes: &mut Vec<String>) {
    if args.public {
        state.public_value = Some(true);
        changes.push("public: true".to_string());
    }
    if args.private {
        state.public_value = Some(false);
        changes.push("public: false".to_string());
    }
}

fn apply_body_flags(
    args: &EditArgs,
    state: &mut ContentEditState,
    changes: &mut Vec<String>,
) -> Result<()> {
    if let Some(b) = &args.body {
        state.body = b.clone();
        changes.push("body: updated".to_string());
    }
    if let Some(file) = &args.body_file {
        let path = Path::new(file);
        state.body = fs::read_to_string(path)
            .with_context(|| format!("Failed to read body file: {}", path.display()))?;
        changes.push("body: updated from file".to_string());
    }
    Ok(())
}

async fn apply_set_value_flags(
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
            state.title = value.to_string();
            changes.push(format!("title: {}", value));
        },
        "description" => {
            state.description = value.to_string();
            changes.push(format!("description: {}", value));
        },
        "keywords" => {
            state.keywords = value.to_string();
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
        changes.push("image: cleared".to_string());
    } else {
        state.image = Some(value.to_string());
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
        state.category_id = Some(None);
        changes.push("category_id: cleared".to_string());
        return Ok(());
    }
    let cat_id = CategoryId::new(value.to_string());
    if !repo.category_exists(&cat_id).await? {
        return Err(anyhow!(
            "Category '{}' not found. Please use an existing category ID.",
            value
        ));
    }
    state.category_id = Some(Some(cat_id));
    changes.push(format!("category_id: {}", value));
    Ok(())
}

fn apply_kind_field(value: &str, state: &mut ContentEditState, changes: &mut Vec<String>) -> Result<()> {
    if !VALID_KINDS.contains(&value) {
        return Err(anyhow!(
            "Invalid kind '{}'. Must be one of: {}",
            value,
            VALID_KINDS.join(", ")
        ));
    }
    state.kind_value = Some(value.to_string());
    changes.push(format!("kind: {}", value));
    Ok(())
}

fn apply_public_field(value: &str, state: &mut ContentEditState, changes: &mut Vec<String>) -> Result<()> {
    let p = value.parse::<bool>().map_err(|_| {
        anyhow!(
            "Invalid boolean value for public: '{}'. Use true or false",
            value
        )
    })?;
    state.public_value = Some(p);
    changes.push(format!("public: {}", p));
    Ok(())
}

fn prompt_content_selection(
    repo: &ContentRepository,
    source: Option<&str>,
    _config: &CliConfig,
) -> Result<String> {
    let rt = tokio::runtime::Handle::current();
    let contents = rt.block_on(async {
        if let Some(source) = source {
            let source = SourceId::new(source.to_string());
            repo.list_by_source_limited(&source, 50).await
        } else {
            repo.list(50, 0).await
        }
    })?;

    if contents.is_empty() {
        return Err(anyhow!("No content found"));
    }

    let items: Vec<String> = contents
        .iter()
        .map(|c| format!("{} - {} ({})", c.id.as_str(), c.title, c.source_id.as_str()))
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select content to edit")
        .items(&items)
        .default(0)
        .interact()
        .context("Failed to get content selection")?;

    Ok(contents[selection].id.as_str().to_string())
}
