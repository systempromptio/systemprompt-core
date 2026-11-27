//! The gateway model list for this host.
//!
//! The candidates are the ids the profile admits for the user, in profile
//! order, kept only when the model catalog lists them too (when a catalog was
//! fetched). A model served by a provider the instance has not configured is
//! left out; a configured provider wins over an unconfigured one listing the
//! same id; a provider that serves no client family hides its models; an id no
//! provider lists is a `claude` model, the catalog being Anthropic-shaped. The
//! manifest's `host_model_protocols` entry for the host then keeps only the
//! models served to one of its families, on `/v1/messages` when `claude` is
//! among them, else on `/v1/chat/completions`. Without an entry the host lists
//! [`DEFAULT_MODEL_FAMILIES`]: the harness speaks both gateway protocols. Names
//! and sizes come from the profile's limits and the catalog, the first positive
//! value winning.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;

use sp_protocol::names::is_plain_id;
use sp_protocol::origin::normalize_instance_url;
use sp_protocol::wire::catalog::{ModelCatalog, Profile, ProviderHealth, base_model_id};

use crate::input::ProjectionInput;
use crate::model::{Gateway, GatewayModel, Protocol};

pub const DEFAULT_MODEL_FAMILIES: [&str; 2] = [CLAUDE, OPENAI];

const CLAUDE: &str = "claude";
const OPENAI: &str = "openai";

pub(crate) fn project(input: &ProjectionInput<'_>, warnings: &mut Vec<String>) -> Gateway {
    let Some(profile) = input.profile else {
        return Gateway::default();
    };
    let base = match normalize_instance_url(input.instance_url) {
        Ok(base) => base,
        Err(error) => {
            tracing::warn!(%error, "instance URL refused for model calls; no gateway models");
            warnings.push(
                "no gateway models: the instance URL is not https or loopback http".to_owned(),
            );
            return Gateway::default();
        },
    };
    let allowed: Vec<String> = input.manifest.model_families(input.host).map_or_else(
        || {
            DEFAULT_MODEL_FAMILIES
                .iter()
                .map(|family| (*family).to_owned())
                .collect()
        },
        <[String]>::to_vec,
    );
    let catalog = input.catalog.filter(|catalog| !catalog.data.is_empty());
    let mut seen: HashSet<&str> = HashSet::new();
    let mut models = Vec::new();
    for id in &profile.models {
        let id = id.as_str();
        if !seen.insert(id) {
            continue;
        }
        if !is_plain_id(id) {
            warnings.push(format!(
                "gateway model {id:?} skipped: its id is not a plain id"
            ));
            continue;
        }
        if let Some(model) = listed(profile, catalog, &allowed, id) {
            models.push(model);
        }
    }
    let default_model = profile
        .default_model
        .as_deref()
        .map(base_model_id)
        .filter(|default| models.iter().any(|model| model.id == *default))
        .map(str::to_owned);
    Gateway {
        base_url: Some(format!("{base}/v1")),
        models,
        default_model,
        protocols: allowed,
    }
}

fn listed(
    profile: &Profile,
    catalog: Option<&ModelCatalog>,
    allowed: &[String],
    id: &str,
) -> Option<GatewayModel> {
    let entry = catalog.and_then(|catalog| catalog.get(id));
    if catalog.is_some() && entry.is_none() {
        tracing::debug!(
            model = id,
            "admitted by the profile but not in the catalog; not listed"
        );
        return None;
    }
    let families = served_families(&profile.providers, id)?;
    let protocol = route(&families, allowed)?;
    let limits = profile
        .model_limits
        .iter()
        .find(|(key, _)| key.as_str() == id)
        .map(|(_, limits)| *limits);
    let name = entry
        .and_then(|entry| entry.display_name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(id)
        .to_owned();
    Some(GatewayModel {
        id: id.to_owned(),
        name,
        protocol,
        context: first_positive([
            limits.map(|limits| u64::from(limits.context_window)),
            entry.and_then(|entry| entry.max_input_tokens),
        ]),
        output: first_positive([
            limits.map(|limits| u64::from(limits.max_output_tokens)),
            entry.and_then(|entry| entry.max_tokens),
        ]),
        families,
    })
}

fn served_families(providers: &[ProviderHealth], id: &str) -> Option<Vec<String>> {
    let serving: Vec<&ProviderHealth> = providers
        .iter()
        .filter(|provider| provider.models.iter().any(|model| model == id))
        .collect();
    let chosen = serving
        .iter()
        .find(|provider| provider.configured)
        .or_else(|| serving.first())
        .copied();
    let Some(provider) = chosen else {
        return Some(vec![CLAUDE.to_owned()]);
    };
    if !provider.configured {
        return None;
    }
    Some(
        provider
            .clients
            .iter()
            .map(|family| family.as_tag().to_owned())
            .collect(),
    )
}

fn route(families: &[String], allowed: &[String]) -> Option<Protocol> {
    let admitted = |family: &str| {
        families.iter().any(|served| served == family)
            && allowed.iter().any(|listed| listed == family)
    };
    if admitted(CLAUDE) {
        Some(Protocol::Messages)
    } else if admitted(OPENAI) {
        Some(Protocol::ChatCompletions)
    } else {
        None
    }
}

fn first_positive(values: [Option<u64>; 2]) -> Option<u64> {
    values.into_iter().flatten().find(|value| *value > 0)
}
