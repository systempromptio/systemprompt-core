//! Disabling the `ai` member withdraws the members that require it, `mcp`
//! among them (it requires the ai member's tool-call intent claims), while
//! the gateway stays admitted: the composition without ai boots the API
//! application with the gateway and without mcp.

use std::sync::Arc;

use systemprompt::{CompositionError, reference};
use systemprompt_api::services::server::routes::Composed;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_identifiers::ExtensionId;
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
async fn disabling_ai_withdraws_mcp_and_keeps_the_gateway_booting() -> anyhow::Result<()> {
    let disabled = without_ai();
    assert!(
        disabled.contains(&ExtensionId::new("mcp")),
        "mcp requires ai, so disabling ai must withdraw it: {disabled:?}"
    );
    assert!(
        !disabled.contains(&ExtensionId::new("gateway")),
        "the gateway stays admitted with ai disabled: {disabled:?}"
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

    drop(ctx);
    drop(pool);
    database.drop_now().await;
    Ok(())
}
