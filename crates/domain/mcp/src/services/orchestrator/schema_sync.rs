use anyhow::Result;
use std::sync::Arc;
use systemprompt_runtime::AppContext;

use crate::services::schema::{SchemaValidationMode, SchemaValidationReport, SchemaValidator};
use crate::McpServerConfig;

pub async fn validate_schemas(
    servers: &[McpServerConfig],
    app_context: &Arc<AppContext>,
) -> Result<()> {
    let schema_report = validate_and_migrate_schemas(servers, app_context).await?;

    report_schema_errors(&schema_report)?;

    if schema_report.created > 0 {
        tracing::debug!("Created {} missing tables", schema_report.created);
    }

    Ok(())
}

fn report_schema_errors(report: &SchemaValidationReport) -> Result<()> {
    if report.errors.is_empty() {
        return Ok(());
    }

    for error in &report.errors {
        tracing::error!(error = %error, "Schema validation error");
    }

    Err(anyhow::anyhow!(
        "Schema validation failed with {} errors",
        report.errors.len()
    ))
}

pub async fn validate_and_migrate_schemas(
    servers: &[McpServerConfig],
    app_context: &Arc<AppContext>,
) -> Result<SchemaValidationReport> {
    let validator = create_schema_validator(app_context)?;
    let mut combined_report = SchemaValidationReport::new("all".to_string());

    for server in servers.iter().filter(|s| !s.schemas.is_empty()) {
        validate_server_schemas(server, &validator, &mut combined_report).await;
    }

    Ok(combined_report)
}

fn create_schema_validator(app_context: &Arc<AppContext>) -> Result<SchemaValidator<'_>> {
    use systemprompt_loader::ConfigLoader;

    let services_config = ConfigLoader::load()?;
    let validation_mode =
        SchemaValidationMode::from_string(&services_config.settings.schema_validation_mode);

    Ok(SchemaValidator::new(
        app_context.db_pool().as_ref(),
        validation_mode,
    ))
}

async fn validate_server_schemas(
    server: &McpServerConfig,
    validator: &SchemaValidator<'_>,
    report: &mut SchemaValidationReport,
) {
    let service_path = std::path::Path::new(&server.crate_path);

    match validator
        .validate_and_apply(&server.name, service_path, &server.schemas)
        .await
    {
        Ok(server_report) => {
            log_successful_validation(server, &server_report);
            report.merge(server_report);
        },
        Err(e) => {
            report.errors.push(format!(
                "Schema validation failed for {}: {}",
                server.name, e
            ));
            tracing::error!(
                service_name = %server.name,
                failure_reason = %e,
                "Schema validation failed"
            );
        },
    }
}

fn log_successful_validation(server: &McpServerConfig, report: &SchemaValidationReport) {
    if report.validated > 0 {
        tracing::info!(
            service_name = %server.name,
            validated = report.validated,
            created = report.created,
            "Validated schemas for MCP service"
        );
    }
}
