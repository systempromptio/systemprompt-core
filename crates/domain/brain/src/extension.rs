//! [`BrainExtension`]: registers the brain tables and migrations, joins the
//! user purge, and depends on the `users` extension.
//!
//! It declares no runner service: the brain consumer needs the pipeline
//! seams the runtime builder assembles, so the builder starts it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_extension::prelude::*;
use systemprompt_traits::Job;

use crate::analyses::BrainAnalysisJob;
use crate::jobs::{BrainRetentionJob, BrainReviewJob};
use crate::plane::BrainPlane;
use crate::schema::BRAIN_SCHEMAS;

#[derive(Debug, Clone, Copy, Default)]
pub struct BrainExtension;

impl Extension for BrainExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "brain",
            name: "Brain",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        BRAIN_SCHEMAS
            .iter()
            .map(|(table, sql, columns)| {
                SchemaDefinition::new(*table, *sql)
                    .with_required_columns(columns.iter().map(|c| (*c).into()).collect())
            })
            .collect()
    }

    fn dependencies(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn cross_extension_tables(&self) -> Vec<&'static str> {
        vec!["users"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn config_planes(&self) -> Vec<Arc<dyn ConfigPlane>> {
        vec![Arc::new(BrainPlane)]
    }

    fn jobs(&self) -> Vec<Arc<dyn Job>> {
        vec![
            Arc::new(BrainReviewJob),
            Arc::new(BrainRetentionJob),
            Arc::new(BrainAnalysisJob),
        ]
    }
}

register_extension!(BrainExtension);

user_purge_tables!(
    "systemprompt-brain",
    [
        (
            "brain_facts",
            "owner_user_id",
            "source_kind = 'session' OR source_kind = 'entered_by' OR source_kind = 'run'"
        ),
        ("brain_recalls", "principal_user_id"),
        ("brain_interests", "owner_user_id"),
    ]
);
