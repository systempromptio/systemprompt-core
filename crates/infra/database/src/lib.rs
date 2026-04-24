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
    ArtifactId, ClientId, ColumnInfo, ContentId, ContextId, DatabaseInfo, DatabaseQuery,
    DatabaseTransaction, DbValue, ExecutionStepId, FileId, FromDatabaseRow, FromDbValue, IndexInfo,
    JsonRow, LogId, QueryResult, QueryRow, QuerySelector, SessionId, SkillId, TableInfo, TaskId,
    ToDbValue, TokenId, TraceId, UserId, parse_database_datetime,
};

pub use services::{
    BoxFuture, Database, DatabaseCliDisplay, DatabaseExt, DatabaseProvider, DatabaseProviderExt,
    DbPool, PostgresProvider, SqlExecutor, with_transaction, with_transaction_raw,
    with_transaction_retry,
};

pub use error::RepositoryError;
pub use lifecycle::{
    AppliedMigration, MigrationResult, MigrationService, MigrationStatus, ModuleInstaller,
    install_extension_schemas, install_extension_schemas_with_config,
    install_module_schemas_from_source, install_module_seeds_from_path, install_schema,
    install_seed, validate_column_exists, validate_database_connection, validate_table_exists,
};
pub use repository::{
    CleanupRepository, CreateServiceInput, DatabaseInfoRepository, PgDbPool, ServiceConfig,
    ServiceRepository,
};

pub use admin::{
    AdminSql, AdminSqlError, DEFAULT_READONLY_ROW_LIMIT, DatabaseAdminService, IdentifierError,
    QueryExecutor, QueryExecutorError, SafeIdentifier,
};
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
