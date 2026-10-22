//! Folding legacy provider entries into credentials, providers and endpoints.
//!
//! Credentials: one platform credential per distinct `api_key_secret` (its id
//! is the secret name) and one personal credential per credential owner (the
//! entry `credential_from` names, else the entry itself), so a stored personal
//! key keeps its row key. Providers: entries sharing one credential where one
//! entry's name prefixes the others' (`vertex`, `vertex-anthropic`, …) are one
//! provider named after that entry, each entry one endpoint; a lone entry is
//! its own provider with endpoint `api`. Sibling entries must agree on the
//! vendor-account facts (user agent, forwarded and extra headers, enabled).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::{CredentialId, EndpointId, ProviderId, SecretName};
use systemprompt_manifest::services::catalog::{
    BetaSpec, CredentialEntry, CredentialRegistry, EndpointConfig, EndpointRef, ProviderConfig,
    ProviderRegistry,
};
use systemprompt_wire::WireProtocol;

use super::abort::{AI_FILE, MigrateAbort, MigrateResult, PROVIDERS_FILE};
use super::legacy::{LegacyAi, LegacyProvider};

/// Where one legacy entry went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryTarget {
    pub endpoint: EndpointRef,
    pub credential: CredentialId,
}

#[derive(Debug, Default)]
pub struct Folded {
    pub credentials: CredentialRegistry,
    pub providers: ProviderRegistry,
    pub entries: BTreeMap<String, EntryTarget>,
}

fn credential_of(entry: &LegacyProvider) -> MigrateResult<(CredentialId, CredentialEntry)> {
    if entry.is_personal() {
        let owner = entry.credential_owner();
        return Ok((
            CredentialId::new(owner),
            CredentialEntry::personal(entry.display_name.clone()),
        ));
    }
    let secret = entry.api_key_secret.as_deref().ok_or_else(|| {
        MigrateAbort::new(
            PROVIDERS_FILE,
            [format!("providers[{}].api_key_secret", entry.name)],
            "a platform entry names no api_key_secret, so no credential can be derived",
        )
    })?;
    Ok((
        CredentialId::new(secret),
        CredentialEntry::platform(SecretName::new(secret)),
    ))
}

const fn wire_endpoint_id(wire: WireProtocol) -> &'static str {
    match wire {
        WireProtocol::Anthropic => "messages",
        WireProtocol::OpenAiChat => "chat",
        WireProtocol::OpenAiResponses => "responses",
        WireProtocol::Gemini => "gemini",
    }
}

fn group_root<'a>(group: &[&'a LegacyProvider]) -> Option<&'a LegacyProvider> {
    group.iter().copied().find(|root| {
        group
            .iter()
            .all(|e| e.name == root.name || e.name.starts_with(&format!("{}-", root.name)))
    })
}

fn groups(entries: &[LegacyProvider]) -> MigrateResult<Vec<Vec<&LegacyProvider>>> {
    let mut by_credential: BTreeMap<CredentialId, Vec<&LegacyProvider>> = BTreeMap::new();
    for entry in entries {
        by_credential
            .entry(credential_of(entry)?.0)
            .or_default()
            .push(entry);
    }
    let mut out = Vec::new();
    for (credential, members) in by_credential {
        let mut remaining = members;
        remaining.sort_by_key(|e| e.name.len());
        while let Some(first) = remaining.first().copied() {
            let prefix = format!("{}-", first.name);
            let (group, rest): (Vec<_>, Vec<_>) = remaining
                .into_iter()
                .partition(|e| e.name == first.name || e.name.starts_with(&prefix));
            if first.is_personal() && !rest.is_empty() {
                return Err(MigrateAbort::new(
                    PROVIDERS_FILE,
                    rest.iter().map(|e| format!("providers[{}]", e.name)),
                    format!(
                        "personal credential '{credential}' is shared by entries whose names do \
                         not share the prefix '{}'; one personal subscription must be one provider",
                        first.name
                    ),
                ));
            }
            out.push(group);
            remaining = rest;
        }
    }
    Ok(out)
}

fn agree<T: PartialEq + std::fmt::Debug>(
    root: &LegacyProvider,
    sibling: &LegacyProvider,
    field: &str,
    get: impl Fn(&LegacyProvider) -> T,
) -> MigrateResult<()> {
    if get(root) == get(sibling) {
        return Ok(());
    }
    Err(MigrateAbort::new(
        PROVIDERS_FILE,
        [
            format!("providers[{}].{field}", root.name),
            format!("providers[{}].{field}", sibling.name),
        ],
        format!(
            "sibling entries of one vendor account disagree on `{field}` ({:?} vs {:?}); make them \
             equal or give the entries different credentials",
            get(root),
            get(sibling)
        ),
    ))
}

fn enabled(ai: &LegacyAi, entry: &LegacyProvider) -> bool {
    ai.providers.get(&entry.name).is_none_or(|p| p.enabled)
}

fn check_siblings(
    root: &LegacyProvider,
    group: &[&LegacyProvider],
    ai: &LegacyAi,
) -> MigrateResult<()> {
    for sibling in group {
        agree(root, sibling, "user_agent", |e| e.user_agent.clone())?;
        agree(root, sibling, "forward_headers", |e| {
            e.forward_headers.clone()
        })?;
        agree(root, sibling, "extra_headers", |e| {
            e.extra_headers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<BTreeMap<_, _>>()
        })?;
        if enabled(ai, root) != enabled(ai, sibling) {
            return Err(MigrateAbort::new(
                AI_FILE,
                [
                    format!("ai.providers.{}.enabled", root.name),
                    format!("ai.providers.{}.enabled", sibling.name),
                ],
                "sibling entries of one vendor account disagree on `enabled`; a provider is \
                 enabled or disabled as a whole",
            ));
        }
    }
    Ok(())
}

fn provider_config(
    root: &LegacyProvider,
    group: &[&LegacyProvider],
    ai: &LegacyAi,
    credential: &CredentialId,
) -> ProviderConfig {
    let mut provider = ProviderConfig::new(credential.as_str());
    provider.display_name.clone_from(&root.display_name);
    provider.description.clone_from(&root.description);
    provider.user_agent.clone_from(&root.user_agent);
    provider.forward_headers.clone_from(&root.forward_headers);
    provider.extra_headers.clone_from(&root.extra_headers);
    provider.governance = root.governance;
    provider.enabled = enabled(ai, root);
    provider.google_search_enabled = group.iter().any(|e| {
        e.wire == WireProtocol::Gemini
            && ai
                .providers
                .get(&e.name)
                .is_some_and(|p| p.google_search_enabled)
    });
    provider
}

pub fn fold(entries: &[LegacyProvider], ai: &LegacyAi) -> MigrateResult<Folded> {
    let mut folded = Folded::default();
    for group in groups(entries)? {
        let root = group_root(&group).unwrap_or(group[0]);
        check_siblings(root, &group, ai)?;
        let (credential_id, credential) = credential_of(root)?;
        folded
            .credentials
            .credentials
            .entry(credential_id.clone())
            .or_insert(credential);
        let provider_id = ProviderId::new(&root.name);
        let mut provider = provider_config(root, &group, ai, &credential_id);
        for entry in &group {
            let endpoint_id = endpoint_id(root, entry, group.len())?;
            if provider.endpoints.contains_key(endpoint_id.as_str()) {
                return Err(MigrateAbort::new(
                    PROVIDERS_FILE,
                    [format!("providers[{}]", entry.name)],
                    format!(
                        "two entries of provider '{provider_id}' map to endpoint '{endpoint_id}'"
                    ),
                ));
            }
            let mut endpoint = EndpointConfig::new(entry.wire, entry.endpoint.clone());
            endpoint.accepted_betas = entry.accepted_betas.clone().map(BetaSpec::List);
            endpoint.resilience = ai.providers.get(&entry.name).and_then(|p| p.resilience);
            provider.endpoints.insert(endpoint_id.clone(), endpoint);
            folded.entries.insert(
                entry.name.clone(),
                EntryTarget {
                    endpoint: EndpointRef {
                        provider: provider_id.clone(),
                        endpoint: endpoint_id,
                    },
                    credential: credential_id.clone(),
                },
            );
        }
        folded.providers.providers.insert(provider_id, provider);
    }
    Ok(folded)
}

fn endpoint_id(
    root: &LegacyProvider,
    entry: &LegacyProvider,
    size: usize,
) -> MigrateResult<EndpointId> {
    if size == 1 {
        return Ok(EndpointId::new("api"));
    }
    if entry.name == root.name {
        return Ok(EndpointId::new(wire_endpoint_id(entry.wire)));
    }
    let suffix = entry
        .name
        .strip_prefix(&format!("{}-", root.name))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            MigrateAbort::new(
                PROVIDERS_FILE,
                [format!("providers[{}]", entry.name)],
                "entry name does not extend its provider's name",
            )
        })?;
    Ok(EndpointId::new(suffix))
}
