//! The `identity` config plane: `services/identity/*.yaml` projected into the
//! users, their roles and service accounts, and static OAuth clients.
//!
//! Every YAML file under `services/identity/` is one part of the declaration;
//! an entity declared in two files is refused. A user entity is keyed
//! `user:<email>` and carries its name, account kind (`person` or `service`,
//! so a service account is a declared user of kind `service`), roles (the
//! manual role grants) and status. An OAuth client entity is keyed
//! `oauth_client:<client_id>` and carries its name, owner (a user's email),
//! redirect URIs, grant types, scopes and the secrets-document key its secret
//! is read from; the secret itself never appears in the declaration or in a
//! row, only its bcrypt hash. API keys are not part of this plane.
//!
//! The plane reads and writes only the rows a declaration stamped
//! (`declared_source`), so users created by SSO, the console or the CLI are
//! never taken over or removed by an apply. Removing an entity from the files
//! releases its row: the stamp is cleared and the user or client deactivated.
//! A client's secret is hashed from the secrets document at write time, which
//! the host provides as an `Arc<Secrets>` dependency; rotating a secret is
//! done by pointing `secret_key` at a new key, since the declaration compares
//! the key name, not the secret.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod declaration;
mod files;

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::PlaneTransaction;
use systemprompt_extension::prelude::{
    ApplyOutcome, ConfigPlane, ConfigPlaneResult, DeclaredEntities, DeclaredState, PlaneContext,
    PlaneSnapshot, PlaneTx, ResolvedEntities,
};
use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_manifest::secrets::Secrets;
use systemprompt_oauth::repository::{ClientRepository, DeclaredClientWrite};
use systemprompt_oauth::services::hash_client_secret;
use systemprompt_users::{DeclaredUserWrite, UserRepository};

use files::{backend, encode, invalid};
pub use files::{declared_entities, render_document};

use declaration::{CLIENT_PREFIX, ClientValue, USER_PREFIX, UserValue, client_key, user_key};

pub const IDENTITY_PLANE: &str = "identity";
pub const IDENTITY_DIR: &str = "identity";

#[derive(Debug, Clone, Copy, Default)]
pub struct IdentityPlane;

#[derive(Clone, Copy)]
struct Declared<'a> {
    value: Option<&'a str>,
    source: &'a str,
}

impl IdentityPlane {
    async fn write_user(
        tx: &mut PlaneTransaction,
        email: &str,
        value: Option<&str>,
        source: &str,
        outcome: &mut ApplyOutcome,
    ) -> ConfigPlaneResult<()> {
        let Some(value) = value else {
            UserRepository::release_declared_on(tx, email)
                .await
                .map_err(backend)?;
            outcome.deleted += 1;
            return Ok(());
        };
        let user: UserValue = serde_json::from_str(value).map_err(backend)?;
        UserRepository::upsert_declared_on(
            tx,
            &DeclaredUserWrite {
                email,
                name: &user.name,
                account_kind: user.kind.as_str(),
                roles: &user.roles,
                status: user.status.as_str(),
                source,
            },
        )
        .await
        .map_err(backend)?;
        outcome.updated += 1;
        Ok(())
    }

    async fn write_client(
        tx: &mut PlaneTransaction,
        secrets: Option<&Secrets>,
        client_id: ClientId,
        declared: Declared<'_>,
        outcome: &mut ApplyOutcome,
    ) -> ConfigPlaneResult<()> {
        let Declared { value, source } = declared;
        let Some(value) = value else {
            ClientRepository::release_declared_on(tx, &client_id)
                .await
                .map_err(backend)?;
            outcome.deleted += 1;
            return Ok(());
        };
        let client: ClientValue = serde_json::from_str(value).map_err(backend)?;
        let owner = UserRepository::find_id_by_email_on(tx, &client.owner)
            .await
            .map_err(backend)?
            .ok_or_else(|| {
                invalid(
                    IDENTITY_DIR,
                    &format_args!(
                        "oauth client {client_id}: owner {} is not a user; declare it under \
                         `users`",
                        client.owner
                    ),
                )
            })?;
        let secret_hash = match client.secret_key.as_deref() {
            None => None,
            Some(key) => {
                let secret = secrets.and_then(|s| s.get(key)).ok_or_else(|| {
                    invalid(
                        IDENTITY_DIR,
                        &format_args!(
                            "oauth client {client_id}: secret_key '{key}' is not in the \
                                 secrets document"
                        ),
                    )
                })?;
                Some(hash_client_secret(secret).map_err(backend)?)
            },
        };
        ClientRepository::upsert_declared_on(
            tx,
            &DeclaredClientWrite {
                client_id: &client_id,
                client_name: &client.name,
                owner_user_id: &owner,
                is_active: client.active,
                secret_key: client.secret_key.as_deref(),
                secret_hash: secret_hash.as_deref(),
                redirect_uris: &client.redirect_uris,
                grant_types: &client.grant_types,
                scopes: &client.scopes,
                source,
            },
        )
        .await
        .map_err(backend)?;
        outcome.updated += 1;
        Ok(())
    }
}

#[async_trait]
impl ConfigPlane for IdentityPlane {
    fn name(&self) -> &'static str {
        IDENTITY_PLANE
    }

    fn declared_path(&self) -> &'static str {
        IDENTITY_DIR
    }

    fn supports_scoped_apply(&self) -> bool {
        true
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        Ok(declared_entities(services_root)?.map(|entities| {
            DeclaredState::from_entries(
                entities
                    .into_iter()
                    .map(|(key, entity)| (key, entity.value))
                    .collect(),
            )
        }))
    }

    fn sources(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredEntities>> {
        declared_entities(services_root)
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        let tx = tx.get::<PlaneTransaction>()?;
        let mut snapshot = PlaneSnapshot::new();
        for row in UserRepository::list_declared_on(tx)
            .await
            .map_err(backend)?
        {
            let value = match (row.account_kind.parse(), row.status.parse()) {
                (Ok(kind), Ok(status)) => encode(&UserValue {
                    name: row.name,
                    kind,
                    roles: declaration::sorted_unique(&row.roles),
                    status,
                })?,
                _ => format!("{}:{}", row.account_kind, row.status),
            };
            snapshot.insert(user_key(&row.email), value);
        }
        let clients = ClientRepository::list_declared_on(tx)
            .await
            .map_err(backend)?;
        let owner_ids: Vec<UserId> = clients.iter().map(|c| c.owner_user_id.clone()).collect();
        let owners: HashMap<UserId, String> = UserRepository::emails_by_ids_on(tx, &owner_ids)
            .await
            .map_err(backend)?
            .into_iter()
            .collect();
        for row in clients {
            let owner = owners
                .get(&row.owner_user_id)
                .cloned()
                .unwrap_or_else(|| row.owner_user_id.as_str().to_owned());
            let value = encode(&ClientValue {
                name: row.client_name,
                owner,
                redirect_uris: row.redirect_uris,
                grant_types: row.grant_types,
                scopes: row.scopes,
                secret_key: row.secret_key,
                active: row.is_active,
            })?;
            snapshot.insert(client_key(row.client_id.as_str()), value);
        }
        Ok(snapshot)
    }

    async fn write(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
        entities: &ResolvedEntities,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        let secrets: Option<&Secrets> = ctx.get::<Arc<Secrets>>().ok().map(|s| &**s);
        let tx = tx.get::<PlaneTransaction>()?;
        let mut outcome = ApplyOutcome::default();
        let (users, clients): (BTreeMap<_, _>, BTreeMap<_, _>) = entities
            .iter()
            .partition(|(key, _)| key.starts_with(USER_PREFIX));
        for (key, resolved) in users {
            let email = key.trim_start_matches(USER_PREFIX);
            let source = resolved.source.to_string();
            Self::write_user(tx, email, resolved.value.as_deref(), &source, &mut outcome).await?;
        }
        for (key, resolved) in clients {
            let Some(client_id) = key.strip_prefix(CLIENT_PREFIX) else {
                return Err(invalid(
                    IDENTITY_DIR,
                    &format_args!("'{key}' is not an identity entity"),
                ));
            };
            let source = resolved.source.to_string();
            Self::write_client(
                tx,
                secrets,
                ClientId::new(client_id),
                Declared {
                    value: resolved.value.as_deref(),
                    source: &source,
                },
                &mut outcome,
            )
            .await?;
        }
        Ok(outcome)
    }

    async fn export(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<Option<String>> {
        let current = self.read_current(ctx, tx).await?;
        if current.is_empty() {
            return Ok(None);
        }
        self.render_declarations(&current).map(Some)
    }

    fn render_declarations(&self, entities: &PlaneSnapshot) -> ConfigPlaneResult<String> {
        render_document(entities)
    }
}
