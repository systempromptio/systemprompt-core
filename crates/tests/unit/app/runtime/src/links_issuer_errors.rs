//! How the MCP side reports a link request the API did not answer with a
//! link: a missing or misrouted endpoint names the status it got instead of
//! claiming a signing failure, and only a 500 reads as one.

use systemprompt_identifiers::{DocumentUploadId, JwtToken, McpServerId};
use systemprompt_runtime::documents::links::{
    ApiLinkIssuer, LinkIssueError, LinkIssuer, LinkRefusal, LinkTarget,
};

fn refused(status: u16) -> LinkIssueError {
    LinkIssueError::Refused {
        status,
        refusal: LinkRefusal::from_status(status),
    }
}

#[test]
fn statuses_map_to_their_refusals() {
    assert_eq!(LinkRefusal::from_status(401), LinkRefusal::HopRefused);
    assert_eq!(LinkRefusal::from_status(403), LinkRefusal::Forbidden);
    assert_eq!(LinkRefusal::from_status(409), LinkRefusal::NotOpen);
    assert_eq!(LinkRefusal::from_status(500), LinkRefusal::Failed);
    for status in [404, 405, 502, 503, 504] {
        assert_eq!(
            LinkRefusal::from_status(status),
            LinkRefusal::Unavailable,
            "{status}"
        );
    }
    assert_eq!(LinkRefusal::Unavailable.code(), "link_endpoint_unavailable");
}

#[test]
fn a_missing_route_names_the_status_not_a_signing_failure() {
    let message = refused(404).tool_message("upload links");
    assert_eq!(
        message,
        "the link endpoint returned 404, so the upload links could not be issued"
    );
    assert!(!message.contains("signed"));
}

#[test]
fn a_refusal_names_its_code() {
    assert_eq!(
        refused(403).tool_message("download link"),
        "the link endpoint refused the download link: forbidden"
    );
    assert_eq!(
        refused(500).tool_message("download link"),
        "the API could not sign the download link"
    );
}

#[tokio::test]
async fn an_unreachable_endpoint_says_so() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    drop(listener);
    let issuer =
        ApiLinkIssuer::new(&format!("http://{addr}"), McpServerId::new("docs")).expect("issuer");
    let error = issuer
        .issue(
            Some(&JwtToken::new("hop")),
            LinkTarget::Upload {
                upload_id: DocumentUploadId::generate(),
            },
        )
        .await
        .expect_err("nothing listens");
    assert!(matches!(error, LinkIssueError::Transport(_)), "{error:?}");
    assert_eq!(
        error.tool_message("upload links"),
        "the link endpoint was unreachable, so the upload links could not be issued"
    );
}
