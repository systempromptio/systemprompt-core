//! The one write path every `admin config credential|provider|endpoint|model|
//! deployment` edit takes.
//!
//! [`apply`] reads the catalog document from `services/ai/`, hands it to the
//! edit (which mutates through `ProviderCatalogService`, so a result that does
//! not resolve is refused before anything is written), commits the files and
//! re-loads the services tree ([`commit_catalog`]), then reconciles the authz
//! catalog so a new route id is governed without a restart.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::kernel::KernelHost;
use std::collections::HashMap;

use anyhow::{Result, anyhow};
use systemprompt_manifest::services::catalog::{CatalogContext, CatalogDocument, EndpointRef};
use systemprompt_models::artifacts::ListItem;
use systemprompt_models::providers::ClientFamily;
use systemprompt_wire::WireProtocol;

use super::services_io::{CatalogLocation, commit_catalog};
use super::types::ConfigMutationOutput;
use systemprompt_extension::cli::CommandOutput;

pub async fn apply(
    host: &dyn KernelHost,
    title: &str,
    edit: impl FnOnce(&mut CatalogDocument, &CatalogContext) -> Result<String>,
) -> Result<CommandOutput> {
    let boot = crate::commands::admin::state::boot(host)?;
    let loaded = boot.loaded_profile();
    let location = CatalogLocation::active(loaded);
    let before = location.read()?;
    let mut after = before.clone();
    let message = edit(
        &mut after,
        &systemprompt_loader::services_state::catalog_context(loaded.profile()),
    )?;
    let services = commit_catalog(&location, &after, loaded.profile())?;
    let outcome = super::reconcile::reconcile_authz(
        crate::commands::admin::state::config(host)?.as_ref(),
        &services,
        &location.dir.display().to_string(),
    )
    .await;
    Ok(CommandOutput::card_value(
        title,
        &ConfigMutationOutput {
            field: "catalog".to_owned(),
            message: super::reconcile::append_reconcile_notice(message, &outcome),
        },
    ))
}

#[must_use]
pub(crate) fn list_output(title: &str, rows: Vec<String>) -> CommandOutput {
    let items: Vec<ListItem> = rows
        .into_iter()
        .map(|row| ListItem::new(row, String::new(), String::new()))
        .collect();
    CommandOutput::list(items).with_title(title)
}

pub async fn read_active(host: &dyn KernelHost) -> Result<CatalogDocument> {
    let boot = crate::commands::admin::state::boot(host)?;
    CatalogLocation::active(boot.loaded_profile()).read()
}

pub(crate) fn parse_wire(raw: &str) -> Result<WireProtocol> {
    WireProtocol::from_tag(raw).ok_or_else(|| {
        anyhow!(
            "invalid --wire '{raw}'; expected one of: anthropic, openai-chat, openai-responses, \
             gemini"
        )
    })
}

pub(crate) fn parse_clients(raw: &[String]) -> Result<Vec<ClientFamily>> {
    raw.iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|tag| !tag.is_empty() && *tag != "none")
        .map(|tag| {
            ClientFamily::from_tag(tag).ok_or_else(|| {
                anyhow!("invalid client '{tag}'; expected claude, openai, gemini or none")
            })
        })
        .collect()
}

pub(crate) fn parse_endpoints(raw: &[String]) -> Result<Vec<EndpointRef>> {
    raw.iter()
        .map(|value| EndpointRef::parse(value).map_err(|e| anyhow!(e)))
        .collect()
}

pub(crate) fn parse_headers(raw: &[String]) -> Result<HashMap<String, String>> {
    raw.iter()
        .map(|h| {
            h.split_once('=')
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .ok_or_else(|| anyhow!("invalid --header '{h}'; expected KEY=VALUE"))
        })
        .collect()
}
