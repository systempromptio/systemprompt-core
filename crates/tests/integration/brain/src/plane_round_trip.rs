use systemprompt_brain::pipeline::BatchContext;
use systemprompt_brain::plane::load_declared_state;
use systemprompt_identifiers::PipelineVersion;

use crate::harness::Harness;
use crate::plane::{self, PRICING, declared};

async fn version(h: &Harness) -> PipelineVersion {
    let mut conn = h.pg.acquire().await.expect("acquire");
    BatchContext::load(&mut conn)
        .await
        .expect("batch context")
        .version()
        .clone()
}

#[tokio::test]
async fn an_applied_plane_reads_back_and_exports_what_was_declared() {
    let h = Harness::new("brain_plane_round_trip").await;

    let mut conn = h.pg.acquire().await.expect("acquire");
    let loaded = load_declared_state(&mut conn).await.expect("load");
    drop(conn);

    assert_eq!(
        loaded.entities().expect("encode"),
        declared().entities().expect("encode")
    );
    assert_eq!(
        plane::read_current(&h.pg).await,
        declared().entities().expect("encode")
    );
    let exported = plane::export(&h.pg)
        .await
        .expect("a populated plane exports");
    assert!(exported.contains(PRICING), "{exported}");
    h.finish().await;
}

#[tokio::test]
async fn editing_a_kind_changes_the_pipeline_version_and_reapplying_does_not() {
    let h = Harness::new("brain_plane_version").await;
    let before = version(&h).await;

    plane::apply(&h.pg, &declared()).await;
    assert_eq!(
        version(&h).await,
        before,
        "negative control: an identical apply keeps the version"
    );

    let mut edited = declared();
    edited.kinds[0].statement = "{subject} costs {price} in {region}".to_owned();
    plane::apply(&h.pg, &edited).await;

    assert_ne!(version(&h).await, before);
    h.finish().await;
}
