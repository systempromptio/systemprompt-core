//! The `gateway` member: the `gateway_policies` config plane, the gateway
//! slots and the one engines set of the process.
//!
//! The gateway owns no tables; the plane writes `ai_gateway_policies` through
//! the AI domain's repository. Its slots are in [`crate::contract`]; the
//! member opens each one, so every gateway slot value is built under core
//! standing and no other slot can build the same value.
//!
//! After governance it provides `Arc<GatewayEngines>` **sealed**: the
//! upstream, scanner, override and route-selector slot values, the gateway
//! guards, the kernel's sealed connector token cache and provider-key lookup,
//! and, while the profile enables the brain, the `Arc<dyn MemoryRecall>` the
//! `brain` member provides (read optionally: disabling the brain leaves the
//! engines without recall). The engines hold the provider secrets, so only
//! kernel code (the API's gateway routes) and core provide fns read them.
//! Their presence is the gateway's one owner check. Building them registers
//! the gateway's safety scanners on the mcp member's artifact ingest, when that
//! member is admitted, so every tool result is scanned by the installation's
//! policy.
//!
//! It builds the bridge routes' narrow views
//! ([`crate::surface::views`]) from the sealed signing keys, secret lookup,
//! configuration, system admin, services root and event router, and provides
//! each sealed, conferred on `gateway-http` under its own row
//! (`gateway-manifest-signer`, `bridge-release-source`, `gateway-settings`,
//! `bridge-event-stream`); no route state reads the seeds themselves. A host
//! that does not seed what a view is built from (a child host, a probe
//! composition) resolves that view as `None`, and `gateway-http` then builds
//! no bridge routes.
//!
//! It also provides [`GatewayAtRest`] **sealed**, conferred on `gateway-http`:
//! the accounting journal under the storage data directory and the at-rest
//! cipher, both keyed from the `encryption_master_key` it reads from the
//! sealed secret store here, so no route state reads the store. A host that
//! does not hold the key resolves a withheld value whose journal does not
//! open.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_ai::repository::AiGatewayPolicyRepository;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_events::EventRouter;
use systemprompt_extension::brain::MemoryRecall;
use systemprompt_extension::mcp::artifacts::DynToolResultIngest;
use systemprompt_extension::prelude::*;
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::{BrainProfile, UnappliedPlaneMode};
use systemprompt_manifest::services::SystemAdmin;
use systemprompt_security::at_rest::MasterKey;
use systemprompt_security::credential::TokenCache;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{CacheEpochView, SecretLookup, SecretStore};

use crate::artifact_scanner::GatewayArtifactScanner;
use crate::at_rest::{GatewayAtRest, JournalFiles};
use crate::surface::views::{
    BridgeEventStream, BridgeReleaseSource, GatewayManifestSigner, GatewaySettingsSource,
    GatewaySettingsView,
};
use systemprompt_extension::DynSealedFiles;

use crate::contract::{OutboundSlot, OverrideSlot, RouteSelectorSlot, SafetyScannerSlot};
use crate::policies::GatewayPoliciesPlane;
use crate::registry::GatewayEngines;

#[derive(Debug, Clone, Copy, Default)]
pub struct GatewayExtension;

impl Extension for GatewayExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "gateway",
            name: "Gateway",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        c.config_plane(Arc::new(GatewayPoliciesPlane));
        c.slot::<RouteSelectorSlot>();
        c.slot::<OverrideSlot>();
        c.slot::<SafetyScannerSlot>();
        c.slot::<OutboundSlot>();
        c.provide_conferred_after_governance(ConferralRow::GatewayEngines, provide_engines);
        c.provide_conferred(
            ConferralRow::GatewayAtRest,
            |r: &Resolved<'_>| -> Result<Arc<GatewayAtRest>, ProvideError> { Ok(at_rest(r)) },
        );
        c.provide_sealed(|r: &Resolved<'_>| -> Result<DynSealedFiles, ProvideError> {
            Ok(Arc::new(JournalFiles(r.get::<Arc<GatewayAtRest>>()?)))
        });
        c.provide_conferred(
            ConferralRow::GatewayManifestSigner,
            |r: &Resolved<'_>| -> Result<Option<Arc<GatewayManifestSigner>>, ProvideError> {
                Ok(r.optional::<SigningKeys>()
                    .map(|keys| Arc::new(GatewayManifestSigner::new(keys))))
            },
        );
        c.provide_conferred(
            ConferralRow::BridgeReleaseSource,
            |r: &Resolved<'_>| -> Result<Option<Arc<BridgeReleaseSource>>, ProvideError> {
                Ok(r.optional::<Arc<dyn SecretLookup>>()
                    .map(|lookup| Arc::new(BridgeReleaseSource::new(lookup))))
            },
        );
        c.provide_conferred(
            ConferralRow::GatewaySettings,
            |r: &Resolved<'_>| -> Result<Option<Arc<GatewaySettingsView>>, ProvideError> {
                Ok(settings(r))
            },
        );
        c.provide_conferred(
            ConferralRow::BridgeEventStream,
            |r: &Resolved<'_>| -> Result<Option<Arc<BridgeEventStream>>, ProvideError> {
                Ok(r.optional::<Arc<EventRouter>>()
                    .map(|router| Arc::new(BridgeEventStream::new(&router))))
            },
        );
        Ok(())
    }
}

fn settings(r: &Resolved<'_>) -> Option<Arc<GatewaySettingsView>> {
    let config = r.optional::<Arc<Config>>()?;
    let admin = r.optional::<SystemAdmin>()?;
    let paths = r.optional::<Arc<AppPaths>>()?;
    Some(Arc::new(GatewaySettingsView::new(GatewaySettingsSource {
        config: &config,
        system_admin: admin.id().clone(),
        services_root: paths.system().services().to_path_buf(),
        secrets: r.optional::<Arc<dyn SecretLookup>>()?,
    })))
}

fn provide_engines(r: &Resolved<'_>) -> Result<Arc<GatewayEngines>, ProvideError> {
    let tokens = r.get::<Arc<TokenCache>>()?;
    let secrets = r.get::<Arc<dyn SecretLookup>>()?;
    let engines = GatewayEngines::new(tokens, r, secrets)
        .map_err(|error| ProvideError::Failed(Box::new(error)))?;
    let brain_enabled = r
        .optional::<Arc<BrainProfile>>()
        .is_some_and(|profile| profile.enabled);
    let recall = r
        .optional::<Arc<dyn MemoryRecall>>()
        .filter(|_| brain_enabled);
    let engines = Arc::new(match recall {
        Some(recall) => engines.with_memory_recall(recall),
        None => engines,
    });
    register_artifact_scanner(r, &engines)?;
    Ok(engines)
}

fn register_artifact_scanner(
    r: &Resolved<'_>,
    engines: &GatewayEngines,
) -> Result<(), ProvideError> {
    let Some(ingest) = r.optional::<DynToolResultIngest>() else {
        tracing::info!(
            reason = "mcp_member_not_admitted",
            "No artifact ingest; the gateway scanner is not registered"
        );
        return Ok(());
    };
    ingest.register_scanner(Arc::new(GatewayArtifactScanner::over_policy_table(
        AiGatewayPolicyRepository::new(&r.get::<DbPool>()?),
        r.get::<CacheEpochView>()?,
        r.get::<UnappliedPlaneMode>()?,
        Arc::clone(&engines.scanner_registry),
    )));
    Ok(())
}

fn at_rest(r: &Resolved<'_>) -> Arc<GatewayAtRest> {
    let key = match r
        .optional::<Arc<dyn SecretStore>>()
        .ok_or(None)
        .and_then(|store| MasterKey::from_store(&*store).map_err(Some))
    {
        Ok(key) => Some(key),
        Err(error) => {
            tracing::warn!(
                error = ?error,
                "the encryption master key is not held here; the gateway journal does not open"
            );
            None
        },
    };
    Arc::new(GatewayAtRest::new(
        r.optional::<Arc<AppPaths>>()
            .map(|paths| paths.storage().data().to_path_buf()),
        key,
    ))
}
