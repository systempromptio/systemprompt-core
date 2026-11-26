//! A composition with `ai` (and what depends on it) disabled keeps the
//! gateway admitted and boots the API application; `mcp` requires the ai
//! member's tool-call intents, so it is refused with `ai` and no artifact
//! ingest is built.

use std::sync::Arc;

use systemprompt::{CompositionError, reference};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_identifiers::ExtensionId;
use systemprompt_mcp::ArtifactIngest;
use systemprompt_test_fixtures::{
    DisposableDb, ensure_test_bootstrap, fixture_app_context_with_extensions, fixture_config,
    test_boot_profile,
};

fn admit(disabled: &[ExtensionId]) -> Result<systemprompt_extension::Admitted, CompositionError> {
    let mut profile = (*test_boot_profile()).clone();
    profile.extensions.disabled = disabled
        .iter()
        .cloned()
        .map(systemprompt_manifest::DisabledExtension::literal)
        .collect();
    reference().build(&profile)
}

fn without_ai() -> Vec<ExtensionId> {
    let mut disabled = vec![ExtensionId::new("ai")];
    loop {
        match admit(&disabled) {
            Err(
                CompositionError::DisabledDependency { owner, .. }
                | CompositionError::WithheldDependency { owner, .. },
            ) if !disabled.contains(&owner) => {
                disabled.push(owner);
            },
            Err(error) => panic!("admission without ai: {error}"),
            Ok(_) => return disabled,
        }
    }
}

#[tokio::test]
async fn with_ai_disabled_the_gateway_boots_without_mcp() -> anyhow::Result<()> {
    let disabled = without_ai();
    assert!(
        !disabled.contains(&ExtensionId::new("gateway")),
        "the gateway stays admitted with ai disabled: {disabled:?}"
    );
    assert!(
        disabled.contains(&ExtensionId::new("mcp")),
        "mcp requires the ai member's tool-call intents, so it is refused with ai: {disabled:?}"
    );
    ensure_test_bootstrap();
    let database = DisposableDb::with_schema("scanner_without_ai").await;
    let pool = database.test_pool().await;
    let mut config = fixture_config(database.url());
    config.database_url = database.url().to_owned();
    let ctx = fixture_app_context_with_extensions(config, Arc::new(admit(&disabled)?)).await?;
    let _router = setup_api_server(
        &ctx,
        Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )?;

    assert!(
        ctx.kernel().state().find::<Arc<ArtifactIngest>>().is_none(),
        "no artifact ingest is built without mcp"
    );
    drop(ctx);
    drop(pool);
    database.drop_now().await;
    Ok(())
}
