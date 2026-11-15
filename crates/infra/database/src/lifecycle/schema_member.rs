//! A composition member as the schema installer and migration runner see it.
//!
//! The installer works over [`SchemaMember`]s: each admitted member's
//! metadata, schemas, migrations, seeds, retirements and cross-member tables,
//! snapshotted from the admitted composition by [`schema_members`] in
//! composition order. A disabled member is absent from the admitted
//! composition, so its schema is never installed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::{BuiltState, ExtensionMetadata, Migration, SchemaDefinition, Seed};

/// The schema-bearing view of one admitted member.
pub trait SchemaMember: Send + Sync {
    fn metadata(&self) -> ExtensionMetadata;

    fn id(&self) -> &'static str {
        self.metadata().id
    }

    fn version(&self) -> &'static str {
        self.metadata().version
    }

    fn schemas(&self) -> Vec<SchemaDefinition>;
    fn migrations(&self) -> Vec<Migration>;
    fn seeds(&self) -> Vec<Seed>;
    fn retirements(&self) -> Vec<SchemaDefinition>;
    fn cross_extension_tables(&self) -> Vec<&'static str>;

    fn has_schemas(&self) -> bool {
        !self.schemas().is_empty()
    }

    fn has_migrations(&self) -> bool {
        !self.migrations().is_empty()
    }
}

/// An owned snapshot of one member's schema contributions.
#[derive(Debug, Clone)]
pub struct SchemaUnit {
    pub metadata: ExtensionMetadata,
    pub schemas: Vec<SchemaDefinition>,
    pub migrations: Vec<Migration>,
    pub seeds: Vec<Seed>,
    pub retirements: Vec<SchemaDefinition>,
    pub cross_tables: Vec<&'static str>,
}

impl SchemaMember for SchemaUnit {
    fn metadata(&self) -> ExtensionMetadata {
        self.metadata
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        self.schemas.clone()
    }

    fn migrations(&self) -> Vec<Migration> {
        self.migrations.clone()
    }

    fn seeds(&self) -> Vec<Seed> {
        self.seeds.clone()
    }

    fn retirements(&self) -> Vec<SchemaDefinition> {
        self.retirements.clone()
    }

    fn cross_extension_tables(&self) -> Vec<&'static str> {
        self.cross_tables.clone()
    }
}

pub fn schema_members(built: &BuiltState) -> Vec<Arc<dyn SchemaMember>> {
    built
        .members()
        .filter(|member| {
            !member.sink.schemas().is_empty()
                || !member.sink.migrations().is_empty()
                || !member.sink.seeds().is_empty()
                || !member.sink.retirements().is_empty()
        })
        .map(|member| {
            let unit: Arc<dyn SchemaMember> = Arc::new(SchemaUnit {
                metadata: member.metadata,
                schemas: member.sink.schemas().to_vec(),
                migrations: member.sink.migrations().to_vec(),
                seeds: member.sink.seeds().to_vec(),
                retirements: member.sink.retirements().to_vec(),
                cross_tables: member.sink.cross_tables().to_vec(),
            });
            unit
        })
        .collect()
}
