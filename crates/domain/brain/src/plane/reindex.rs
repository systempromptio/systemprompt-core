//! What an apply rebuilds after its entities are written, on the same
//! transaction: a changed topic (its audience widens or narrows every
//! fact carrying it) rebuilds the working set; a changed kind (its
//! identity or entity fields decide the search weights and the links)
//! rebuilds the links and the weighted search vectors; a changed synonyms
//! document rebuilds the vectors. Rebuilds are whole-table and derived
//! from the rows just written, so an apply that changes nothing relevant
//! rebuilds nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_extension::prelude::{ConfigPlaneResult, ResolvedEntities};

use super::backend;
use super::entities::{DOC, KIND, SYNONYMS_DOC, TOPIC};
use crate::repository::{
    FactLinkRepository, SearchIndexRepository, SynonymsRepository, WorkingSetRepository,
};

/// Which derived indexes an apply's entity keys invalidate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rebuilds {
    pub working_set: bool,
    pub links: bool,
    pub search: bool,
}

impl Rebuilds {
    #[must_use]
    pub fn for_keys<K: AsRef<str>>(keys: impl IntoIterator<Item = K>) -> Self {
        let mut rebuilds = Self::default();
        for key in keys {
            let key = key.as_ref();
            let (kind, id) = key.split_once('/').unwrap_or((key, ""));
            match (kind, id) {
                (TOPIC, _) => rebuilds.working_set = true,
                (KIND, _) => {
                    rebuilds.links = true;
                    rebuilds.search = true;
                },
                (DOC, SYNONYMS_DOC) => rebuilds.search = true,
                _ => {},
            }
        }
        rebuilds
    }

    #[must_use]
    pub const fn any(self) -> bool {
        self.working_set || self.links || self.search
    }
}

pub(super) async fn after_write(
    conn: &mut PgConnection,
    entities: &ResolvedEntities,
) -> ConfigPlaneResult<Rebuilds> {
    let rebuilds = Rebuilds::for_keys(entities.keys().map(String::as_str));
    if rebuilds.working_set {
        WorkingSetRepository::rebuild_all(&mut *conn)
            .await
            .map_err(backend)?;
    }
    if rebuilds.links {
        FactLinkRepository::rebuild_all(&mut *conn)
            .await
            .map_err(backend)?;
    }
    if rebuilds.search {
        let synonyms = SynonymsRepository::load(&mut *conn)
            .await
            .map_err(backend)?;
        SearchIndexRepository::rebuild_all(conn, &synonyms)
            .await
            .map_err(backend)?;
    }
    Ok(rebuilds)
}
