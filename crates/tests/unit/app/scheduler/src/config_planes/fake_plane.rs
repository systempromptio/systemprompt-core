use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use systemprompt_provider_contracts::{
    ApplyMode, ApplyOutcome, ChangeRequest, ConfigPlane, ConfigPlaneError, ConfigPlaneResult,
    DeclaredState, PlaneContext, PlaneSnapshot,
};

pub struct FakePlane {
    pub name: &'static str,
    pub path: &'static str,
    pub store: Mutex<PlaneSnapshot>,
    pub converges: bool,
    pub applied_reasons: Mutex<Vec<String>>,
}

pub fn unique_plane_name(prefix: &str) -> &'static str {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    Box::leak(format!("{prefix}_{}_{seq}_{nanos}", std::process::id()).into_boxed_str())
}

impl FakePlane {
    pub fn new(name: &'static str, store: &[(&str, &str)]) -> Self {
        Self {
            name,
            path: Box::leak(format!("planes/{name}.txt").into_boxed_str()),
            store: Mutex::new(
                store
                    .iter()
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            ),
            converges: true,
            applied_reasons: Mutex::new(Vec::new()),
        }
    }

    pub fn stuck(mut self) -> Self {
        self.converges = false;
        self
    }

    pub fn write_declaration(&self, root: &Path, lines: &[(&str, &str)]) {
        let path = root.join(self.path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        let body: String = lines.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        std::fs::write(path, body).expect("write declaration");
    }

    pub fn store(&self) -> PlaneSnapshot {
        self.store.lock().expect("lock").clone()
    }
}

#[async_trait]
impl ConfigPlane for FakePlane {
    fn name(&self) -> &'static str {
        self.name
    }

    fn declared_path(&self) -> &'static str {
        self.path
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
        let mut entries = PlaneSnapshot::new();
        for line in text.lines().filter(|l| !l.is_empty()) {
            let (k, v) = line
                .split_once('=')
                .ok_or_else(|| ConfigPlaneError::Invalid {
                    plane: self.name.to_owned(),
                    path: self.path.to_owned(),
                    message: format!("line without '=': {line}"),
                })?;
            entries.insert(k.to_owned(), v.to_owned());
        }
        Ok(Some(DeclaredState::from_entries(entries)))
    }

    async fn read_current(&self, _ctx: &PlaneContext<'_>) -> ConfigPlaneResult<PlaneSnapshot> {
        Ok(self.store())
    }

    async fn apply(
        &self,
        _ctx: &PlaneContext<'_>,
        declared: &DeclaredState,
        mode: ApplyMode,
        change: &ChangeRequest<'_>,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        self.applied_reasons
            .lock()
            .expect("lock")
            .push(change.reason.to_owned());
        if !self.converges {
            return Ok(ApplyOutcome::default());
        }
        let mut store = self.store.lock().expect("lock");
        let mut outcome = ApplyOutcome::default();
        for (k, v) in &declared.entries {
            match store.get(k) {
                None => {
                    outcome.inserted += 1;
                    store.insert(k.clone(), v.clone());
                },
                Some(have) if have != v && mode == ApplyMode::Overwrite => {
                    outcome.updated += 1;
                    store.insert(k.clone(), v.clone());
                },
                Some(_) => {},
            }
        }
        if mode == ApplyMode::Overwrite {
            let before = store.len();
            store.retain(|k, _| declared.entries.contains_key(k));
            outcome.deleted = before - store.len();
        }
        Ok(outcome)
    }
}
