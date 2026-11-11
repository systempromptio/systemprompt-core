use std::collections::BTreeSet;

use systemprompt_agent::services::agent_orchestration::process::grants::model_credential_secrets;

use crate::services::a2a_server::a2a_helpers::agent_config;

fn catalog_platform_secrets(
    catalog: &systemprompt_manifest::services::catalog::ResolvedCatalog,
) -> BTreeSet<String> {
    catalog
        .credentials()
        .filter_map(|credential| credential.platform_secret())
        .map(|name| name.as_str().to_owned())
        .collect()
}

#[test]
fn an_agent_on_a_model_the_catalog_does_not_serve_receives_no_provider_secret() {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    let mut agent = agent_config("grants_agent");
    agent.metadata.model = Some("no-such-model".to_owned());
    assert!(model_credential_secrets(&agent, bootstrap.services.catalog()).is_empty());
}

#[test]
fn an_agent_on_the_default_model_receives_only_that_model_s_credentials() {
    let bootstrap = systemprompt_test_fixtures::ensure_test_bootstrap();
    let catalog = bootstrap.services.catalog();
    let agent = agent_config("grants_agent");
    let derived: BTreeSet<String> = model_credential_secrets(&agent, catalog)
        .into_iter()
        .collect();
    let expected: BTreeSet<String> = catalog
        .default_model()
        .into_iter()
        .flat_map(|model| catalog.enabled_hops(model.as_str()))
        .filter_map(|hop| hop.credential.platform_secret())
        .map(|name| name.as_str().to_owned())
        .collect();
    assert_eq!(derived, expected);
    assert!(derived.is_subset(&catalog_platform_secrets(catalog)));
}
