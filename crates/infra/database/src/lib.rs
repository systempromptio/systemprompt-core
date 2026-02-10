pub mod admin;
pub mod error;
pub mod extension;
pub mod lifecycle;
pub mod models;
#[macro_use]
pub mod repository;
pub mod services;

pub use extension::DatabaseExtension;

pub use models::{
    parse_database_datetime, ArtifactId, ClientId, ColumnInfo, ContentId, ContextId, DatabaseInfo,
    DatabaseQuery, DatabaseTransaction, DbValue, ExecutionStepId, FileId, FromDatabaseRow,
    FromDbValue, IndexInfo, JsonRow, LogId, QueryResult, QueryRow, QuerySelector, SessionId,
    SkillId, TableInfo, TaskId, ToDbValue, TokenId, TraceId, UserId,
};

pub use services::{
    with_transaction, with_transaction_raw, with_transaction_retry, BoxFuture, Database,
    DatabaseCliDisplay, DatabaseExt, DatabaseProvider, DatabaseProviderExt, DbPool,
    PostgresProvider, SqlExecutor,
};

pub use error::RepositoryError;
pub use lifecycle::{
    install_extension_schemas, install_extension_schemas_with_config,
    install_module_schemas_from_source, install_module_seeds_from_path, install_schema,
    install_seed, validate_column_exists, validate_database_connection, validate_table_exists,
    AppliedMigration, MigrationResult, MigrationService, MigrationStatus, ModuleInstaller,
};
pub use repository::{
    CleanupRepository, CreateServiceInput, DatabaseInfoRepository, PgDbPool, ServiceConfig,
    ServiceRepository,
};

pub use admin::{DatabaseAdminService, QueryExecutor, QueryExecutorError};
pub use sqlx::types::Json;
pub use sqlx::{PgPool, Pool, Postgres, Transaction};

use systemprompt_traits::DatabaseHandle;

impl DatabaseHandle for Database {
    fn is_connected(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
