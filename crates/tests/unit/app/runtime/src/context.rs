//! Unit tests for AppContextBuilder.

use systemprompt_runtime::AppContextBuilder;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_admitted};

#[test]
fn test_context_builder_with_migrations() {
    let builder = AppContextBuilder::new(ensure_test_bootstrap().boot.clone(), fixture_admitted())
        .with_migrations(true);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("install_schemas: true"));
}
