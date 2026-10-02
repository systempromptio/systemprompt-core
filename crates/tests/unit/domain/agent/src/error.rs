//! Unit tests for agent error types
//!
//! Tests cover:
//! - ArtifactError variants and error messages
//! - AgentError conversions and wrapping

use std::error::Error as _;

use systemprompt_agent::{AgentError, ArtifactError};
use systemprompt_traits::{MetadataValidationError, RepositoryError};

#[test]
fn test_artifact_error_missing_field_display() {
    let error = ArtifactError::MissingField {
        field: "structured_content".to_string(),
    };
    assert!(error.to_string().contains("Missing required field"));
    assert!(error.to_string().contains("structured_content"));
}

#[test]
fn test_artifact_error_transform_display() {
    let error = ArtifactError::Transform("Failed to convert format".to_string());
    assert!(error.to_string().contains("Transform error"));
    assert!(error.to_string().contains("Failed to convert format"));
}

#[test]
fn test_artifact_error_metadata_validation_keeps_the_cause() {
    let error: ArtifactError = MetadataValidationError::new("context_id", "is empty").into();
    assert!(matches!(error, ArtifactError::MetadataValidation(_)));
    assert!(error.to_string().contains("Metadata validation error"));
    assert!(error.source().is_some());
}

#[test]
fn test_agent_error_artifact_display() {
    let artifact_error = ArtifactError::Transform("bad".to_string());
    let agent_error: AgentError = artifact_error.into();
    assert!(matches!(agent_error, AgentError::Artifact(_)));
    assert!(agent_error.to_string().contains("Artifact error"));
}

#[test]
fn test_agent_error_repository_keeps_its_classification() {
    let agent_error: AgentError = RepositoryError::not_found("task", "t1").into();
    match agent_error {
        AgentError::Repository(inner) => assert!(inner.is_not_found()),
        other => panic!("expected Repository, got {other:?}"),
    }
}

#[test]
fn test_agent_error_from_sqlx_row_not_found_is_not_found() {
    let agent_error: AgentError = sqlx::Error::RowNotFound.into();
    match agent_error {
        AgentError::Repository(inner) => assert!(inner.is_not_found()),
        other => panic!("expected Repository, got {other:?}"),
    }
}
