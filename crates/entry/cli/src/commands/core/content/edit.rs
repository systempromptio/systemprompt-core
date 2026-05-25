use super::edit_apply::{
    ContentEditState, apply_body_flags, apply_set_value_flags, apply_visibility_flags,
};
use super::types::UpdateOutput;
use crate::cli_settings::CliConfig;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use anyhow::{Context, Result, anyhow};
use clap::Args;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use systemprompt_content::{CategoryIdUpdate, ContentRepository};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ContentId, LocaleCode, SourceId};
use systemprompt_logging::CliService;
use systemprompt_runtime::AppContext;

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

pub(super) async fn execute(
    args: EditArgs,
    config: &CliConfig,
) -> Result<CommandResult<UpdateOutput>> {
    let ctx = AppContext::new().await?;
    execute_with_pool(args, ctx.db_pool(), config).await
}

pub(super) async fn execute_with_pool(
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
        repo.get_by_source_and_slug(&source, &identifier, &LocaleCode::new("en"))
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
        category_id: CategoryIdUpdate::Unchanged,
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

fn prompt_content_selection(
    repo: &ContentRepository,
    source: Option<&str>,
    _config: &CliConfig,
) -> Result<String> {
    let rt = tokio::runtime::Handle::current();
    let contents = rt.block_on(async {
        if let Some(source) = source {
            let source = SourceId::new(source.to_string());
            repo.list_by_source_limited(&source, &LocaleCode::new("en"), 50)
                .await
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
