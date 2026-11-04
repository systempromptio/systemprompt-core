use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_api::routes::brain::BrainHttpError;
use systemprompt_brain::BrainError;
use systemprompt_extension::brain::RecallError;
use systemprompt_identifiers::BrainProposalId;
use systemprompt_identifiers::error::IdValidationError;
use systemprompt_models::brain::{StatementError, SubjectKeyError, UnknownFactStatus};
use systemprompt_traits::{AuthProviderError, RepositoryError};

fn key_of(error: BrainHttpError) -> (Option<String>, String) {
    let api = error.into_inner();
    (api.error_key.clone(), api.message.clone())
}

#[test]
fn invalid_subject_and_statement_are_422_with_their_keys() {
    let subject = BrainError::Subject(SubjectKeyError::Shape("bad subject".to_owned()));
    let status = BrainHttpError::from(subject).into_response().status();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let oversized = BrainError::Subject(SubjectKeyError::IdLength { len: 300 });
    let (key, _) = key_of(BrainHttpError::from(oversized));
    assert_eq!(key.as_deref(), Some("invalid_subject"));

    let (key, _) = key_of(BrainHttpError::from(BrainError::Statement(
        StatementError::MissingField("owner".to_owned()),
    )));
    assert_eq!(key.as_deref(), Some("invalid_statement"));

    let (key, _) = key_of(BrainHttpError::from(BrainError::InvalidFields(
        "field price is not declared".to_owned(),
    )));
    assert_eq!(key.as_deref(), Some("invalid_fields"));
}

#[test]
fn an_unknown_proposal_is_404() {
    let id = BrainProposalId::new("00000000-0000-4000-8000-0000000000b1");
    let error = BrainHttpError::from(BrainError::ProposalNotFound(id));
    let (key, message) = key_of(error);
    assert_eq!(key.as_deref(), Some("proposal_not_found"));
    assert!(message.contains("proposal"), "{message}");
}

#[test]
fn a_repository_conflict_is_409() {
    let error = BrainError::Repository(RepositoryError::conflict("fact", "f-1", "duplicate"));
    let (key, _) = key_of(BrainHttpError::from(error));
    assert_eq!(key.as_deref(), Some("conflict"));
    let status = BrainHttpError::from(BrainError::Repository(RepositoryError::conflict(
        "fact",
        "f-1",
        "duplicate",
    )))
    .into_response()
    .status();
    assert_eq!(status, StatusCode::CONFLICT);
}

#[test]
fn invalid_repository_data_is_422() {
    let error = BrainError::Repository(RepositoryError::invalid_data("fields", "not json"));
    let (key, _) = key_of(BrainHttpError::from(error));
    assert_eq!(key.as_deref(), Some("invalid_params"));
}

#[test]
fn a_non_transient_failure_is_a_fixed_500() {
    let error = BrainError::UnknownEvent("brain.mystery".to_owned());
    let status = BrainHttpError::from(error).into_response().status();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    let (_, message) = key_of(BrainHttpError::from(BrainError::UnknownEvent(
        "brain.secret-detail".to_owned(),
    )));
    assert!(!message.contains("secret-detail"), "{message}");
}

#[test]
fn a_spent_budget_is_503_unavailable() {
    let (key, _) = key_of(BrainHttpError::from(BrainError::BudgetExhausted));
    assert_eq!(key.as_deref(), Some("brain_unavailable"));
}

#[test]
fn auth_lookup_failure_is_a_fixed_500() {
    let error = BrainHttpError::from(AuthProviderError::UserNotFound);
    let status = error.into_response().status();
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[test]
fn recall_failure_is_503_recall_unavailable() {
    let recall = RecallError::Repository(RepositoryError::database(std::io::Error::other("down")));
    let (key, message) = key_of(BrainHttpError::from(recall));
    assert_eq!(key.as_deref(), Some("recall_unavailable"));
    assert!(!message.contains("down"), "{message}");
}

#[test]
fn subject_status_and_id_parse_failures_are_422() {
    let cases = [
        (
            BrainHttpError::from(SubjectKeyError::Shape("x".to_owned())),
            "invalid_subject",
        ),
        (
            BrainHttpError::from(UnknownFactStatus("dreaming".to_owned())),
            "invalid_status",
        ),
        (
            BrainHttpError::from(IdValidationError::empty("BrainFactId")),
            "invalid_id",
        ),
    ];
    for (error, expected) in cases {
        let (key, _) = key_of(error);
        assert_eq!(key.as_deref(), Some(expected));
    }
    let status = BrainHttpError::from(UnknownFactStatus("dreaming".to_owned()))
        .into_response()
        .status();
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
