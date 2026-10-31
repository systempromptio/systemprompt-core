use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_provider_contracts::{
    ApplyOutcome, ApplyRequest, ConfigPlane, ConfigPlaneResult, ConfigPlaneSet, DeclaredState,
    EffectiveAuthority, PlaneAuthority, PlaneContext, PlaneSnapshot, PlaneTx,
};

struct NamedPlane(&'static str);

#[async_trait]
impl ConfigPlane for NamedPlane {
    fn name(&self) -> &'static str {
        self.0
    }

    fn declared_path(&self) -> &'static str {
        "planes/named.yaml"
    }

    fn load_declared(&self, _root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        Ok(None)
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        Ok(PlaneSnapshot::new())
    }

    async fn apply(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
        _request: &ApplyRequest<'_>,
    ) -> ConfigPlaneResult<ApplyOutcome> {
        Ok(ApplyOutcome::default())
    }
}

#[test]
fn authorities_render_their_wire_names() {
    assert_eq!(PlaneAuthority::File.as_str(), "file");
    assert_eq!(PlaneAuthority::Db.as_str(), "db");
    assert_eq!(PlaneAuthority::default(), PlaneAuthority::Db);
    assert_eq!(EffectiveAuthority::File.as_str(), "file");
    assert_eq!(EffectiveAuthority::Db.as_str(), "db");
    assert_eq!(EffectiveAuthority::Undeclared.as_str(), "undeclared");
    assert!(EffectiveAuthority::File.is_file_managed());
    assert!(!EffectiveAuthority::Db.is_file_managed());
    assert!(!EffectiveAuthority::Undeclared.is_file_managed());
}

#[test]
fn plane_authority_round_trips_lowercase() {
    let parsed: BTreeMap<String, PlaneAuthority> =
        serde_yaml::from_str("users: file\nroles: db\n").expect("yaml");
    assert_eq!(parsed["users"], PlaneAuthority::File);
    assert_eq!(parsed["roles"], PlaneAuthority::Db);
    assert!(serde_yaml::from_str::<PlaneAuthority>("disk").is_err());
}

#[test]
fn debug_output_names_planes_and_authority_without_internals() {
    let mut authority = BTreeMap::new();
    authority.insert("alpha_plane".to_owned(), PlaneAuthority::File);
    let set = ConfigPlaneSet::new(
        vec![
            Arc::new(NamedPlane("alpha_plane")),
            Arc::new(NamedPlane("beta_plane")),
        ],
        authority,
        "/nonexistent/services".into(),
    )
    .expect("set");
    let rendered = format!("{set:?}");
    assert!(
        rendered.contains("alpha_plane") && rendered.contains("beta_plane"),
        "{rendered}"
    );
    assert!(rendered.contains("/nonexistent/services"), "{rendered}");

    let writer = set.writer("beta_plane").expect("undeclared is writable");
    assert_eq!(writer.plane().name(), "beta_plane");
    assert_eq!(writer.plane().declared_path(), "planes/named.yaml");
    let rendered = format!("{writer:?}");
    assert!(rendered.contains("beta_plane"), "{rendered}");
    assert!(rendered.contains("Undeclared"), "{rendered}");
}
