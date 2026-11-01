use systemprompt_extension::{LoaderError, SchemaDefinition};
use systemprompt_test_fixtures::install_extension_schemas_with_config;

use super::installation::{
    StubExtension, drop_table, provider_and_db, registry_with, table_exists, unique_id,
};

#[tokio::test]
async fn a_failing_schema_statement_rolls_the_whole_phase_back() {
    let (provider, db) = provider_and_db().await;
    let table = unique_id("phase_fail");
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{table}\" (id BIGINT PRIMARY KEY, label \
         no_such_type_xyz);"
    );
    let ext = StubExtension {
        id: unique_id("ext_phase_fail"),
        schemas: vec![SchemaDefinition::new(table, sql)],
        seeds: vec![],
        migrations: vec![],
    };

    let err = install_extension_schemas_with_config(&registry_with(ext), &provider, &[])
        .await
        .expect_err("the column type does not exist");
    let rendered = format!("{err}");
    let context = match &err {
        LoaderError::SchemaInstallationStepFailed { context, .. } => context.clone(),
        other => panic!("unexpected {other:?}: {rendered}"),
    };
    assert!(context.contains("failed"), "{context}");
    assert!(context.contains("no_such_type_xyz"), "{context}");
    assert!(
        !table_exists(&db, table).await,
        "a failed phase leaves no table behind"
    );
    drop_table(&db, table).await;
}
