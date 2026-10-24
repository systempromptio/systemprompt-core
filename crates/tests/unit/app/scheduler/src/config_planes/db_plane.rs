use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_provider_contracts::{
    ApplyMode, ApplyOutcome, ApplyRequest, ConfigPlane, ConfigPlaneError, ConfigPlaneResult,
    DeclaredState, PlaneContext, PlaneEffect, PlaneSnapshot, PlaneTx, ResolvedEntities,
};
use systemprompt_scheduler::repository::PlaneTransaction;

pub type ApplyLog = Arc<Mutex<Vec<&'static str>>>;

pub struct DbPlane {
    pub name: &'static str,
    pub path: &'static str,
    pub deps: &'static [&'static str],
    pub effect: PlaneEffect,
    pub corrupt: bool,
    pub log: ApplyLog,
}

impl DbPlane {
    pub fn new(name: &'static str, log: &ApplyLog) -> Self {
        Self {
            name,
            path: Box::leak(format!("planes/{name}.txt").into_boxed_str()),
            deps: &[],
            effect: PlaneEffect::Live,
            corrupt: false,
            log: Arc::clone(log),
        }
    }

    pub fn write_declaration(&self, root: &Path, lines: &[(&str, &str)]) {
        let path = root.join(self.path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        let body: String = lines.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        std::fs::write(path, body).expect("write declaration");
    }

    fn backend(&self, source: sqlx::Error) -> ConfigPlaneError {
        ConfigPlaneError::Backend {
            plane: self.name.to_owned(),
            source: Box::new(source),
        }
    }
}

pub async fn ensure_table(pool: &DbPool) {
    let pg = pool.write_pool();
    let mut tx = pg.begin().await.expect("begin");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('plane_e2e_rows'))")
        .execute(&mut *tx)
        .await
        .expect("ddl lock");
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS plane_e2e_rows (plane TEXT NOT NULL, entity TEXT NOT NULL, \
         value TEXT NOT NULL, PRIMARY KEY (plane, entity))",
    )
    .execute(&mut *tx)
    .await
    .expect("create plane_e2e_rows");
    tx.commit().await.expect("commit");
}

pub async fn seed_rows(pool: &DbPool, plane: &str, rows: &[(&str, &str)]) {
    let pg = pool.write_pool();
    for (entity, value) in rows {
        sqlx::query("INSERT INTO plane_e2e_rows (plane, entity, value) VALUES ($1, $2, $3)")
            .bind(plane)
            .bind(entity)
            .bind(value)
            .execute(&*pg)
            .await
            .expect("seed row");
    }
}

pub async fn stored_rows(pool: &DbPool, plane: &str) -> PlaneSnapshot {
    let pg = pool.pool();
    sqlx::query_as::<_, (String, String)>(
        "SELECT entity, value FROM plane_e2e_rows WHERE plane = $1",
    )
    .bind(plane)
    .fetch_all(&*pg)
    .await
    .expect("read rows")
    .into_iter()
    .collect()
}

#[async_trait]
impl ConfigPlane for DbPlane {
    fn name(&self) -> &'static str {
        self.name
    }

    fn declared_path(&self) -> &'static str {
        self.path
    }

    fn depends_on(&self) -> &'static [&'static str] {
        self.deps
    }

    fn effect(&self) -> PlaneEffect {
        self.effect
    }

    fn supports_scoped_apply(&self) -> bool {
        true
    }

    fn load_declared(&self, services_root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        let path = services_root.join(self.path);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).map_err(|e| ConfigPlaneError::Unreadable {
            plane: self.name.to_owned(),
            path: self.path.to_owned(),
            source: Box::new(e),
        })?;
        let entries = text
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
        Ok(Some(DeclaredState::from_entries(entries)))
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        let tx = tx.get::<PlaneTransaction>()?;
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT entity, value FROM plane_e2e_rows WHERE plane = $1",
        )
        .bind(self.name)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| self.backend(e))?;
        Ok(rows.into_iter().collect())
    }

    async fn apply(
        &self,
        ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
        request: &ApplyRequest<'_>,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        self.log.lock().expect("lock").push(self.name);
        let current = self.read_current(ctx, tx).await?;
        let tx = tx.get::<PlaneTransaction>()?;
        let mut outcome = ApplyOutcome::default();
        for (entity, value) in &request.declared.entries {
            if !request.scope.admits(entity) {
                continue;
            }
            let value = if self.corrupt {
                "corrupted"
            } else {
                value.as_str()
            };
            match current.get(entity) {
                Some(have) if have == value || request.mode == ApplyMode::InsertOnly => continue,
                Some(_) => outcome.updated += 1,
                None => outcome.inserted += 1,
            }
            sqlx::query(
                "INSERT INTO plane_e2e_rows (plane, entity, value) VALUES ($1, $2, $3) ON \
                 CONFLICT (plane, entity) DO UPDATE SET value = EXCLUDED.value",
            )
            .bind(self.name)
            .bind(entity)
            .bind(value)
            .execute(&mut **tx)
            .await
            .map_err(|e| self.backend(e))?;
        }
        if request.mode != ApplyMode::InsertOnly {
            for entity in current.keys() {
                if request.scope.admits(entity) && !request.declared.entries.contains_key(entity) {
                    sqlx::query("DELETE FROM plane_e2e_rows WHERE plane = $1 AND entity = $2")
                        .bind(self.name)
                        .bind(entity)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| self.backend(e))?;
                    outcome.deleted += 1;
                }
            }
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
        Ok(Some(
            current.iter().map(|(k, v)| format!("{k}={v}\n")).collect(),
        ))
    }

    async fn write(
        &self,
        _ctx: &PlaneContext<'_>,
        tx: &mut PlaneTx<'_>,
        entities: &ResolvedEntities,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        self.log.lock().expect("lock").push(self.name);
        let tx = tx.get::<PlaneTransaction>()?;
        let mut outcome = ApplyOutcome::default();
        for (entity, resolved) in entities {
            match &resolved.value {
                Some(value) => {
                    let value = if self.corrupt {
                        "corrupted"
                    } else {
                        value.as_str()
                    };
                    sqlx::query(
                        "INSERT INTO plane_e2e_rows (plane, entity, value) VALUES ($1, $2, $3) \
                         ON CONFLICT (plane, entity) DO UPDATE SET value = EXCLUDED.value",
                    )
                    .bind(self.name)
                    .bind(entity)
                    .bind(value)
                    .execute(&mut **tx)
                    .await
                    .map_err(|e| self.backend(e))?;
                    outcome.updated += 1;
                },
                None => {
                    sqlx::query("DELETE FROM plane_e2e_rows WHERE plane = $1 AND entity = $2")
                        .bind(self.name)
                        .bind(entity)
                        .execute(&mut **tx)
                        .await
                        .map_err(|e| self.backend(e))?;
                    outcome.deleted += 1;
                },
            }
        }
        Ok(outcome)
    }

    fn render_declarations(&self, entities: &PlaneSnapshot) -> ConfigPlaneResult<String> {
        Ok(entities.iter().map(|(k, v)| format!("{k}={v}\n")).collect())
    }
}
