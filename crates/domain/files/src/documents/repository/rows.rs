use std::str::FromStr;

use systemprompt_models::documents::{PresenceSource, PresenceState, UnknownDocumentValue};
use systemprompt_traits::RepositoryError;

pub(super) type RepoResult<T> = Result<T, RepositoryError>;

pub(super) fn parse<T>(field: &'static str, value: &str) -> RepoResult<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    T::from_str(value).map_err(|source| RepositoryError::decode(field, source))
}

pub(super) const fn presence_state_str(state: PresenceState) -> &'static str {
    match state {
        PresenceState::Viewing => "viewing",
        PresenceState::Editing => "editing",
    }
}

pub(super) const fn presence_source_str(source: PresenceSource) -> &'static str {
    match source {
        PresenceSource::Bridge => "bridge",
        PresenceSource::Agent => "agent",
        PresenceSource::Web => "web",
    }
}

pub(super) fn parse_presence_state(value: &str) -> RepoResult<PresenceState> {
    match value {
        "viewing" => Ok(PresenceState::Viewing),
        "editing" => Ok(PresenceState::Editing),
        other => Err(unknown("document_presence.state", "presence state", other)),
    }
}

pub(super) fn parse_presence_source(value: &str) -> RepoResult<PresenceSource> {
    match value {
        "bridge" => Ok(PresenceSource::Bridge),
        "agent" => Ok(PresenceSource::Agent),
        "web" => Ok(PresenceSource::Web),
        other => Err(unknown(
            "document_presence.source",
            "presence source",
            other,
        )),
    }
}

fn unknown(field: &'static str, kind: &'static str, value: &str) -> RepositoryError {
    RepositoryError::decode(
        field,
        UnknownDocumentValue {
            kind,
            value: value.to_owned(),
        },
    )
}
