//! Integration tests for the `systemprompt-files` crate.
//!
//! `DATABASE_URL` must point at a Postgres instance with the full
//! systemprompt-core schema applied (run `systemprompt-test-migrate` first).

#[cfg(test)]
mod bootstrap;
#[cfg(test)]
mod config_paths;
#[cfg(test)]
mod documents_access_requests;
#[cfg(test)]
mod documents_agent_caps;
#[cfg(test)]
mod documents_agent_context;
#[cfg(test)]
mod documents_agent_fixture;
#[cfg(test)]
mod documents_agent_moves;
#[cfg(test)]
mod documents_agent_rules;
#[cfg(test)]
mod documents_agent_visibility;
#[cfg(test)]
mod documents_blob_store;
#[cfg(test)]
mod documents_collab;
#[cfg(test)]
mod documents_collab_images;
#[cfg(test)]
mod documents_collab_settle;
#[cfg(test)]
mod documents_collab_timer;
#[cfg(test)]
mod documents_comments;
#[cfg(test)]
mod documents_comments_v2;
#[cfg(test)]
mod documents_drive_paths;
#[cfg(test)]
mod documents_engine_review;
#[cfg(test)]
mod documents_folders;
#[cfg(test)]
mod documents_foreign_lock;
#[cfg(test)]
mod documents_lifecycle;
#[cfg(test)]
mod documents_listing;
#[cfg(test)]
mod documents_node_ids_backfill;
#[cfg(test)]
mod documents_notification_facts;
#[cfg(test)]
mod documents_proposals;
#[cfg(test)]
mod documents_proposals_review;
#[cfg(test)]
mod documents_read_pool_lag;
#[cfg(test)]
mod documents_reads;
#[cfg(test)]
mod documents_repository;
#[cfg(test)]
mod documents_revision_engine;
#[cfg(test)]
mod documents_service;
#[cfg(test)]
mod documents_service_drive;
#[cfg(test)]
mod documents_service_fixture;
#[cfg(test)]
mod documents_shared_editing;
#[cfg(test)]
mod documents_sharing_audit;
#[cfg(test)]
mod documents_sharing_guards;
#[cfg(test)]
mod documents_write_acks;
#[cfg(test)]
mod file_ingestion;
#[cfg(test)]
mod repository;
#[cfg(test)]
mod services;
#[cfg(test)]
mod upload_service;
