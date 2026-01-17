use anyhow::{bail, Context, Result};
use systemprompt_core_agent::repository::context::ContextRepository;
use systemprompt_identifiers::{ContextId, UserId};

pub async fn resolve_context(
    identifier: &str,
    user_id: &UserId,
    repo: &ContextRepository,
) -> Result<ContextId> {
    let contexts = repo
        .list_contexts_basic(user_id)
        .await
        .context("Failed to list contexts for resolution")?;

    if let Some(ctx) = contexts
        .iter()
        .find(|c| c.context_id.as_str() == identifier)
    {
        return Ok(ctx.context_id.clone());
    }

    if identifier.len() >= 4 {
        let matches: Vec<_> = contexts
            .iter()
            .filter(|c| c.context_id.as_str().starts_with(identifier))
            .collect();

        match matches.len() {
            0 => {},
            1 => return Ok(matches[0].context_id.clone()),
            _ => {
                let ids: Vec<&str> = matches.iter().map(|c| c.context_id.as_str()).collect();
                bail!(
                    "Ambiguous context ID prefix '{}'. Matches: {}",
                    identifier,
                    ids.join(", ")
                );
            },
        }
    }

    if let Some(ctx) = contexts.iter().find(|c| c.name == identifier) {
        return Ok(ctx.context_id.clone());
    }

    let lower_identifier = identifier.to_lowercase();
    let name_matches: Vec<_> = contexts
        .iter()
        .filter(|c| c.name.to_lowercase() == lower_identifier)
        .collect();

    match name_matches.len() {
        0 => bail!("Context not found: '{}'", identifier),
        1 => Ok(name_matches[0].context_id.clone()),
        _ => {
            let names: Vec<&str> = name_matches.iter().map(|c| c.name.as_str()).collect();
            bail!(
                "Ambiguous context name '{}'. Matches: {}",
                identifier,
                names.join(", ")
            );
        },
    }
}
