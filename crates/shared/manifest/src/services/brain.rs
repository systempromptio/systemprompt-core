//! Where brain declarations live under the services root.
//!
//! `brain/topics.yaml`, `brain/filters.yaml`, `brain/importance.yaml` and
//! `brain/retrieval.yaml` are single documents; `brain/kinds/`,
//! `brain/interests/` and `brain/profiles/` hold one YAML declaration per
//! file. Together they parse into `systemprompt_models::brain::BrainDocs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const BRAIN_DIR: &str = "brain";
pub const TOPICS_FILE: &str = "topics.yaml";
pub const FILTERS_FILE: &str = "filters.yaml";
pub const IMPORTANCE_FILE: &str = "importance.yaml";
pub const RETRIEVAL_FILE: &str = "retrieval.yaml";
pub const KINDS_DIR: &str = "kinds";
pub const INTERESTS_DIR: &str = "interests";
pub const PROFILES_DIR: &str = "profiles";
