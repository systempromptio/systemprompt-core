//! The per-entity plane contract: the closed change-source vocabulary
//! round-trips through its text form and refuses everything else, and the
//! trait defaults attribute a plain declaration to its file and refuse writes
//! a plane did not implement.

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;
use systemprompt_identifiers::UserId;
use systemprompt_provider_contracts::{
    ApplyOutcome, ApplyRequest, ChangeSource, ChangeSourceError, ConfigPlane, ConfigPlaneError,
    ConfigPlaneResult, DeclaredState, Dependencies, PlaneContext, PlaneSnapshot, PlaneTx,
    ResolvedEntities, entity_hash,
};

fn every_source() -> Vec<ChangeSource> {
    vec![
        ChangeSource::Code("services/ai/gateway.yaml".into()),
        ChangeSource::Bundle("dynapps-agent-farm".into()),
        ChangeSource::Console(UserId::new("user_42")),
        ChangeSource::Migration,
    ]
}

#[test]
fn every_source_round_trips_through_its_text_form() {
    for source in every_source() {
        let text = source.to_string();
        assert!(text.starts_with(source.kind()), "{text}");
        assert_eq!(text.parse::<ChangeSource>(), Ok(source));
    }
}

#[test]
fn the_remainder_after_the_first_colon_is_kept_verbatim() {
    let parsed: ChangeSource = "code:services/a:b.yaml".parse().expect("parses");
    assert_eq!(parsed, ChangeSource::Code("services/a:b.yaml".into()));
}

#[test]
fn text_outside_the_vocabulary_is_refused() {
    for text in [
        "yaml",
        "dashboard",
        "profile:local",
        "migration:007",
        "",
        "Code:x",
    ] {
        assert_eq!(
            text.parse::<ChangeSource>(),
            Err(ChangeSourceError::Unknown(text.to_owned())),
            "{text}"
        );
    }
}

#[test]
fn a_prefix_with_nothing_after_it_is_refused() {
    assert_eq!(
        "bundle:".parse::<ChangeSource>(),
        Err(ChangeSourceError::Empty("bundle"))
    );
    assert_eq!(
        "console:".parse::<ChangeSource>(),
        Err(ChangeSourceError::Empty("console"))
    );
}

#[test]
fn serde_uses_the_text_form() {
    for source in every_source() {
        let json = serde_json::to_string(&source).expect("serialises");
        assert_eq!(json, format!("\"{source}\""));
        let back: ChangeSource = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, source);
    }
    assert!(serde_json::from_str::<ChangeSource>("\"yaml\"").is_err());
}

struct FilePlane;

#[async_trait]
impl ConfigPlane for FilePlane {
    fn name(&self) -> &'static str {
        "file_plane"
    }

    fn declared_path(&self) -> &'static str {
        "services/file_plane.yaml"
    }

    fn load_declared(&self, _root: &Path) -> ConfigPlaneResult<Option<DeclaredState>> {
        Ok(Some(DeclaredState::from_entries(BTreeMap::from([
            ("a".to_owned(), "one".to_owned()),
            ("b".to_owned(), "two".to_owned()),
        ]))))
    }

    async fn read_current(
        &self,
        _ctx: &PlaneContext<'_>,
        _tx: &mut PlaneTx<'_>,
    ) -> ConfigPlaneResult<PlaneSnapshot> {
        Ok(PlaneSnapshot::from([("a".to_owned(), "db".to_owned())]))
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
fn default_sources_attribute_each_entry_to_the_declaration_file() {
    let entities = FilePlane
        .sources(Path::new("/unused"))
        .expect("loads")
        .expect("declared");
    assert_eq!(entities.len(), 2);
    let a = &entities["a"];
    assert_eq!(a.value, "one");
    assert_eq!(a.hash, entity_hash("one"));
    assert_eq!(
        a.source,
        ChangeSource::Code("services/file_plane.yaml".into())
    );
    assert_ne!(entities["b"].hash, a.hash);
}

#[test]
fn entity_hash_is_stable_sha256_hex() {
    assert_eq!(
        entity_hash(""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[tokio::test]
async fn default_read_is_read_current_and_write_is_refused() {
    let deps = Dependencies::default();
    let ctx = PlaneContext::new(Path::new("/unused"), &deps);
    let mut handle = ();
    let mut tx = PlaneTx::new(&mut handle);
    let read = FilePlane.read(&ctx, &mut tx).await.expect("reads");
    assert_eq!(read.get("a").map(String::as_str), Some("db"));

    let written = FilePlane
        .write(&ctx, &mut tx, &ResolvedEntities::new())
        .await;
    assert!(matches!(
        written,
        Err(ConfigPlaneError::Unsupported {
            operation: "write",
            ..
        })
    ));
    assert!(matches!(
        FilePlane.render_declarations(&PlaneSnapshot::new()),
        Err(ConfigPlaneError::Unsupported {
            operation: "render_declarations",
            ..
        })
    ));
}
