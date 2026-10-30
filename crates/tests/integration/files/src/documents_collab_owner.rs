use std::sync::Arc;
use std::time::Duration;

use systemprompt_files::documents::collab::{
    CollabService, CollabTuning, FORWARDED_HEADER, ForwardError, ForwardRequest, OwnerConfig,
    UpdateRoute,
};
use systemprompt_files::documents::repository::OwnerClaim;
use systemprompt_identifiers::DocumentId;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::documents_collab::{Collab, collab, html_document, loro};
use crate::documents_service_fixture::person;

fn replica(c: &Collab, address: &str, hold: Duration) -> CollabService {
    CollabService::new(Arc::clone(&c.documents), CollabTuning::default(), loro()).with_owner(
        OwnerConfig {
            address: address.to_owned(),
            hold,
            forward_timeout: Duration::from_secs(2),
        },
    )
}

async fn document(c: &Collab) -> DocumentId {
    html_document(c, &person("owner-doc")).await
}

#[test]
fn owner_config_at_carries_the_default_hold_and_timeout() {
    let config = OwnerConfig::at("http://10.0.0.7:8080");
    assert_eq!(config.address, "http://10.0.0.7:8080");
    assert_eq!(config.hold, Duration::from_secs(15));
    assert_eq!(config.forward_timeout, Duration::from_secs(3));
}

#[tokio::test]
async fn a_replica_without_owner_config_always_applies_locally() {
    let c = collab().await;
    let doc = document(&c).await;
    assert_eq!(c.collab.update_route(&doc).await, UpdateRoute::Local);
}

#[tokio::test]
async fn the_first_replica_owns_and_the_second_forwards_to_it() {
    let c = collab().await;
    let doc = document(&c).await;
    let a = replica(&c, "http://replica-a:8080", Duration::from_secs(30));
    let b = replica(&c, "http://replica-b:8080", Duration::from_secs(30));
    assert_eq!(a.update_route(&doc).await, UpdateRoute::Local);
    assert_eq!(a.update_route(&doc).await, UpdateRoute::Local);
    assert_eq!(
        b.update_route(&doc).await,
        UpdateRoute::Forward {
            address: "http://replica-a:8080".to_owned()
        }
    );
}

#[tokio::test]
async fn a_lapsed_lease_is_taken_over() {
    let c = collab().await;
    let doc = document(&c).await;
    let a = replica(&c, "http://replica-a:8080", Duration::from_millis(200));
    let b = replica(&c, "http://replica-b:8080", Duration::from_secs(30));
    assert_eq!(a.update_route(&doc).await, UpdateRoute::Local);
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(b.update_route(&doc).await, UpdateRoute::Local);
    assert_eq!(
        a.update_route(&doc).await,
        UpdateRoute::Forward {
            address: "http://replica-b:8080".to_owned()
        }
    );
}

#[tokio::test]
async fn two_replicas_advertising_one_address_never_forward_to_themselves() {
    let c = collab().await;
    let doc = document(&c).await;
    let a = replica(&c, "http://shared:8080", Duration::from_secs(30));
    let b = replica(&c, "http://shared:8080", Duration::from_secs(30));
    assert_eq!(a.update_route(&doc).await, UpdateRoute::Local);
    assert_eq!(b.update_route(&doc).await, UpdateRoute::Local);
}

#[tokio::test]
async fn the_repository_claims_renews_steals_and_releases() {
    let c = collab().await;
    let doc = document(&c).await;
    let repo = &c.h.repos.collab;
    let (first, second) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let long = Duration::from_secs(30);
    assert_eq!(
        repo.claim_owner(&doc, first, "http://a", long)
            .await
            .expect("claim"),
        OwnerClaim::Owned
    );
    assert_eq!(
        repo.claim_owner(&doc, first, "http://a", long)
            .await
            .expect("renew"),
        OwnerClaim::Owned
    );
    assert_eq!(
        repo.claim_owner(&doc, second, "http://b", long)
            .await
            .expect("contend"),
        OwnerClaim::Held {
            address: "http://a".to_owned()
        }
    );
    assert!(
        !repo
            .release_owner(&doc, second)
            .await
            .expect("foreign release")
    );
    assert!(repo.release_owner(&doc, first).await.expect("release"));
    assert!(
        !repo
            .release_owner(&doc, first)
            .await
            .expect("released twice")
    );
    assert_eq!(
        repo.claim_owner(&doc, second, "http://b", Duration::from_millis(100))
            .await
            .expect("claim after release"),
        OwnerClaim::Owned
    );
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        repo.claim_owner(&doc, first, "http://a", long)
            .await
            .expect("steal"),
        OwnerClaim::Owned
    );
}

fn request(address: &str) -> ForwardRequest<'_> {
    ForwardRequest {
        address,
        path_and_query: "/api/v1/documents/x/collab/updates?session=s",
        authorization: Some("Bearer token-1"),
        cookie: Some("sid=abc"),
        content_type: Some("application/octet-stream"),
        body: b"update-bytes".to_vec(),
    }
}

#[tokio::test]
async fn forwarding_without_owner_config_is_disabled() {
    let c = collab().await;
    let refused = c.collab.forward_update(request("http://127.0.0.1:9")).await;
    assert!(
        matches!(refused, Err(ForwardError::Disabled)),
        "{refused:?}"
    );
}

#[tokio::test]
async fn an_unreachable_owner_is_a_transport_error() {
    let c = collab().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = format!("http://{}", listener.local_addr().expect("addr"));
    drop(listener);
    let a = replica(&c, "http://replica-a:8080", Duration::from_secs(30));
    let refused = a.forward_update(request(&address)).await;
    assert!(
        matches!(refused, Err(ForwardError::Transport(_))),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_forward_carries_the_raw_request_and_returns_the_owner_answer() {
    let c = collab().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = format!("http://{}/", listener.local_addr().expect("addr"));
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut received = Vec::new();
        let mut buffer = [0_u8; 4096];
        while !String::from_utf8_lossy(&received).contains("update-bytes") {
            let read = socket.read(&mut buffer).await.expect("read");
            if read == 0 {
                break;
            }
            received.extend_from_slice(&buffer[..read]);
        }
        let body = "{\"seq\":4}";
        let response = format!(
            "HTTP/1.1 429 Too Many Requests\r\ncontent-type: application/json\r\nretry-after: \
             2\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.expect("write");
        String::from_utf8_lossy(&received).to_lowercase()
    });
    let a = replica(&c, "http://replica-a:8080", Duration::from_secs(30));
    let answer = a.forward_update(request(&address)).await.expect("forward");
    assert_eq!(answer.status, 429);
    assert_eq!(answer.content_type.as_deref(), Some("application/json"));
    assert_eq!(answer.retry_after.as_deref(), Some("2"));
    assert_eq!(answer.body, b"{\"seq\":4}");
    let seen = server.await.expect("server");
    assert!(
        seen.starts_with("post /api/v1/documents/x/collab/updates?session=s "),
        "{seen}"
    );
    assert!(seen.contains(&format!("{}:", FORWARDED_HEADER.to_lowercase())));
    assert!(seen.contains("authorization: bearer token-1"));
    assert!(seen.contains("cookie: sid=abc"));
    assert!(seen.contains("content-type: application/octet-stream"));
}
