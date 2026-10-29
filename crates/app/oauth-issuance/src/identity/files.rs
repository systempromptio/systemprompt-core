//! Reading the `services/identity/` declaration files into entities, and
//! rendering a snapshot back into one document.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::{Path, PathBuf};

use systemprompt_extension::prelude::{
    ChangeSource, ConfigPlaneError, ConfigPlaneResult, DeclaredEntities, DeclaredEntity,
    PlaneSnapshot,
};
use systemprompt_identifiers::ClientId;

use super::declaration::{
    CLIENT_PREFIX, ClientValue, IdentityDocument, USER_PREFIX, UserValue, client_key, user_key,
};
use super::{IDENTITY_DIR, IDENTITY_PLANE};

pub(super) fn invalid(path: &str, error: &dyn std::fmt::Display) -> ConfigPlaneError {
    ConfigPlaneError::Invalid {
        plane: IDENTITY_PLANE.to_owned(),
        path: path.to_owned(),
        message: error.to_string(),
    }
}

pub(super) fn backend(error: impl std::error::Error + Send + Sync + 'static) -> ConfigPlaneError {
    ConfigPlaneError::Backend {
        plane: IDENTITY_PLANE.to_owned(),
        source: Box::new(error),
    }
}

pub(super) fn unreadable(path: &str, error: std::io::Error) -> ConfigPlaneError {
    ConfigPlaneError::Unreadable {
        plane: IDENTITY_PLANE.to_owned(),
        path: path.to_owned(),
        source: Box::new(error),
    }
}

pub(super) fn declaration_files(
    services_root: &Path,
) -> ConfigPlaneResult<Option<Vec<(String, PathBuf)>>> {
    let dir = services_root.join(IDENTITY_DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(unreadable(IDENTITY_DIR, error)),
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| unreadable(IDENTITY_DIR, e))?.path();
        let is_yaml = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e == "yaml" || e == "yml");
        if !is_yaml || !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        files.push((format!("{IDENTITY_DIR}/{name}"), path.clone()));
    }
    files.sort();
    Ok(Some(files))
}

pub(super) fn encode<T: serde::Serialize>(value: &T) -> ConfigPlaneResult<String> {
    serde_json::to_string(value).map_err(backend)
}

pub fn declared_entities(services_root: &Path) -> ConfigPlaneResult<Option<DeclaredEntities>> {
    let Some(files) = declaration_files(services_root)? else {
        return Ok(None);
    };
    let mut entities = DeclaredEntities::new();
    for (relative, path) in files {
        let text = std::fs::read_to_string(&path).map_err(|e| unreadable(&relative, e))?;
        let doc: IdentityDocument =
            serde_yaml::from_str(&text).map_err(|e| invalid(&relative, &e))?;
        let origin = ChangeSource::Code(relative.clone());
        let mut insert = |key: String, value: String| {
            if entities.contains_key(&key) {
                return Err(invalid(
                    &relative,
                    &format_args!("{key} is declared more than once"),
                ));
            }
            entities.insert(key, DeclaredEntity::new(value, origin.clone()));
            Ok(())
        };
        for user in &doc.users {
            user.validate().map_err(|e| invalid(&relative, &e))?;
            insert(user_key(&user.email), encode(&user.value())?)?;
        }
        for client in &doc.oauth_clients {
            client.validate().map_err(|e| invalid(&relative, &e))?;
            insert(client_key(&client.client_id), encode(&client.value())?)?;
        }
    }
    Ok(Some(entities))
}

pub fn render_document(entities: &PlaneSnapshot) -> ConfigPlaneResult<String> {
    let mut doc = IdentityDocument::default();
    for (key, value) in entities {
        if let Some(email) = key.strip_prefix(USER_PREFIX) {
            let user: UserValue = serde_json::from_str(value).map_err(backend)?;
            doc.users.push(user.into_declared(email));
        } else if let Some(client_id) = key.strip_prefix(CLIENT_PREFIX) {
            let client: ClientValue = serde_json::from_str(value).map_err(backend)?;
            doc.oauth_clients
                .push(client.into_declared(ClientId::new(client_id)));
        }
    }
    serde_yaml::to_string(&doc).map_err(backend)
}
