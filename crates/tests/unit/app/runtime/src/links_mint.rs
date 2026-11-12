//! The API side of link minting: a hop token admits a link only for the
//! server it was minted for, and only to a target its subject may open.

use std::sync::Arc;

use systemprompt_files::uploads::models::NewUpload;
use systemprompt_identifiers::{JwtToken, McpServerId, TraceId, UserId};
use systemprompt_models::auth::{AuthenticatedUser, McpHopIdentity, UserType};
use systemprompt_models::documents::content_hash;
use systemprompt_runtime::documents::download::Rendition;
use systemprompt_runtime::documents::links::{
    IssuedLink, LinkMintError, LinkMinter, LinkRequest, LinkTarget, mint_link,
};
use systemprompt_security::jwt::{encode_mcp_hop, mcp_hop_claims};
use systemprompt_security::keys::{RsaSigningKey, SigningKeys};
use systemprompt_test_fixtures::services_hosting;

use crate::documents_mcp_rig::{Rig, rig};
use crate::documents_mcp_support::{create_text, ctx_for, registered};
use crate::link_rig::{LINKS_BASE, LINKS_ISSUER, hop_for, link_authority};

const SERVER: &str = "documents";

fn minter(r: &Rig) -> LinkMinter {
    minter_hosting(r, SERVER, &["documents"])
}

fn minter_hosting(r: &Rig, server: &str, toolsets: &[&str]) -> LinkMinter {
    LinkMinter {
        tokens: link_authority(),
        documents: Arc::clone(r.runtime.service()),
        uploads: Arc::clone(r.runtime.uploads()),
        services: services_hosting(server, toolsets),
        api_external_url: LINKS_BASE.to_owned(),
    }
}

async fn open_upload(r: &Rig, owner: &UserId) -> LinkTarget {
    let bytes = vec![5u8; 64];
    let session = r
        .runtime
        .uploads()
        .create(
            owner,
            NewUpload {
                name: "a.bin".to_owned(),
                mime: "application/octet-stream".to_owned(),
                size: 64,
                sha256: content_hash(&bytes),
            },
            &TraceId::generate(),
        )
        .await
        .expect("upload session");
    LinkTarget::Upload {
        upload_id: session.id,
    }
}

fn request(target: LinkTarget) -> LinkRequest {
    LinkRequest {
        server: McpServerId::new(SERVER),
        target,
    }
}

fn hop(user: &AuthenticatedUser, server: &str) -> JwtToken {
    hop_for(user, &ctx_for(Some(user)), server)
}

#[tokio::test]
async fn the_owner_of_an_upload_gets_its_links() {
    let r = rig(true).await;
    let owner = registered("mint-up-owner").await;
    let target = open_upload(&r, &owner.id).await;
    let link = mint_link(&minter(&r), hop(&owner, SERVER).as_str(), &request(target))
        .await
        .expect("owner's links");
    let IssuedLink::Upload(links) = link else {
        panic!("an upload link")
    };
    assert!(
        links.parts_url.starts_with(LINKS_BASE),
        "{}",
        links.parts_url
    );
    assert!(!links.token.is_empty());
}

#[tokio::test]
async fn an_upload_another_user_opened_is_refused() {
    let r = rig(true).await;
    let owner = registered("mint-up-a").await;
    let other = registered("mint-up-b").await;
    let target = open_upload(&r, &owner.id).await;
    let refused = mint_link(&minter(&r), hop(&other, SERVER).as_str(), &request(target))
        .await
        .expect_err("not the owner");
    assert!(matches!(refused, LinkMintError::Upload(_)), "{refused:?}");
}

#[tokio::test]
async fn a_revision_the_subject_cannot_see_is_refused_and_the_owner_gets_its_link() {
    let r = rig(true).await;
    let owner = registered("mint-doc-a").await;
    let other = registered("mint-doc-b").await;
    let (document, revision) = create_text(&r.tools, &ctx_for(Some(&owner)), "secret").await;
    let target = LinkTarget::Download {
        document_id: document.clone(),
        revision_id: revision.clone(),
        rendition: Rendition::Native,
    };
    let refused = mint_link(
        &minter(&r),
        hop(&other, SERVER).as_str(),
        &request(target.clone()),
    )
    .await
    .expect_err("not visible to the other user");
    assert!(matches!(refused, LinkMintError::Document(_)), "{refused:?}");

    let IssuedLink::Download { url, .. } =
        mint_link(&minter(&r), hop(&owner, SERVER).as_str(), &request(target))
            .await
            .expect("owner's link")
    else {
        panic!("a download link")
    };
    assert!(
        url.contains(document.as_str()) && url.contains(revision.as_str()),
        "{url}"
    );
}

#[tokio::test]
async fn a_hop_token_minted_for_another_server_is_refused() {
    let r = rig(true).await;
    let owner = registered("mint-foreign-srv").await;
    let target = open_upload(&r, &owner.id).await;
    let refused = mint_link(
        &minter(&r),
        hop(&owner, "another-server").as_str(),
        &request(target),
    )
    .await
    .expect_err("foreign audience");
    assert!(matches!(refused, LinkMintError::Hop(_)), "{refused:?}");
}

#[tokio::test]
async fn a_hop_token_signed_by_a_foreign_key_or_forged_is_refused() {
    let r = rig(true).await;
    let owner = registered("mint-foreign-key").await;
    let target = open_upload(&r, &owner.id).await;
    let foreign =
        SigningKeys::new(RsaSigningKey::generate().expect("key"), &[9; 32]).expect("keys");
    let claims = mcp_hop_claims(
        LINKS_ISSUER,
        &McpServerId::new(SERVER),
        &McpHopIdentity {
            user_id: owner.id.clone(),
            user: Some(owner.clone()),
            session_id: systemprompt_identifiers::SessionId::new("s"),
            user_type: UserType::User,
            client_id: None,
            act_chain: Vec::new(),
            purpose: None,
            interaction: None,
            mcp_session: None,
        },
    );
    let token = encode_mcp_hop(&foreign, &claims).expect("encode");
    for presented in [token.as_str(), "not-a-token"] {
        let refused = mint_link(&minter(&r), presented, &request(target.clone()))
            .await
            .expect_err("foreign key");
        assert!(matches!(refused, LinkMintError::Hop(_)), "{refused:?}");
    }
}

#[tokio::test]
async fn an_anonymous_hop_is_refused() {
    let r = rig(true).await;
    let owner = registered("mint-anon").await;
    let target = open_upload(&r, &owner.id).await;
    let identity = McpHopIdentity {
        user_id: owner.id.clone(),
        user: None,
        session_id: systemprompt_identifiers::SessionId::new("s"),
        user_type: UserType::Anon,
        client_id: None,
        act_chain: Vec::new(),
        purpose: None,
        interaction: None,
        mcp_session: None,
    };
    let token = link_authority()
        .mint_mcp_hop(&McpServerId::new(SERVER), &identity)
        .expect("hop");
    let refused = mint_link(&minter(&r), &token, &request(target))
        .await
        .expect_err("anonymous");
    assert!(matches!(refused, LinkMintError::Anonymous), "{refused:?}");
}

fn request_for(server: &str, target: LinkTarget) -> LinkRequest {
    LinkRequest {
        server: McpServerId::new(server),
        target,
    }
}

#[tokio::test]
async fn a_server_hosting_neither_documents_nor_issues_is_refused_every_link() {
    let r = rig(true).await;
    let owner = registered("mint-not-hosted").await;
    let upload = open_upload(&r, &owner.id).await;
    let (document, revision) = create_text(&r.tools, &ctx_for(Some(&owner)), "plain").await;
    let download = LinkTarget::Download {
        document_id: document,
        revision_id: revision,
        rendition: Rendition::Native,
    };
    for toolsets in [&[][..], &["admin"][..]] {
        let minter = minter_hosting(&r, "plain-srv", toolsets);
        for target in [upload.clone(), download.clone()] {
            let refused = mint_link(
                &minter,
                hop(&owner, "plain-srv").as_str(),
                &request_for("plain-srv", target),
            )
            .await
            .expect_err("the server hosts no link toolset");
            assert!(
                matches!(refused, LinkMintError::NotHosted { .. }),
                "{refused:?}"
            );
        }
    }
}

#[tokio::test]
async fn an_issues_server_gets_upload_links_but_never_download_links() {
    let r = rig(true).await;
    let owner = registered("mint-issues-only").await;
    let upload = open_upload(&r, &owner.id).await;
    let (document, revision) = create_text(&r.tools, &ctx_for(Some(&owner)), "notes").await;
    let minter = minter_hosting(&r, "issues-srv", &["issues"]);
    let hop = hop(&owner, "issues-srv");
    let link = mint_link(&minter, hop.as_str(), &request_for("issues-srv", upload))
        .await
        .expect("an issues server may sign an upload");
    assert!(matches!(link, IssuedLink::Upload(_)), "{link:?}");
    let refused = mint_link(
        &minter,
        hop.as_str(),
        &request_for(
            "issues-srv",
            LinkTarget::Download {
                document_id: document,
                revision_id: revision,
                rendition: Rendition::Native,
            },
        ),
    )
    .await
    .expect_err("an issues server signs no download");
    assert!(
        matches!(refused, LinkMintError::NotHosted { .. }),
        "{refused:?}"
    );
}

#[tokio::test]
async fn an_issues_server_is_refused_links_to_an_upload_already_completed() {
    let r = rig(true).await;
    let owner = registered("mint-issues-closed").await;
    let bytes = vec![3u8; 48];
    let done = r
        .runtime
        .uploads()
        .put_inline(
            &owner.id,
            NewUpload {
                name: "done.bin".to_owned(),
                mime: "application/octet-stream".to_owned(),
                size: 48,
                sha256: content_hash(&bytes),
            },
            bytes,
            &TraceId::generate(),
        )
        .await
        .expect("inline upload");
    let minter = minter_hosting(&r, "issues-srv", &["issues"]);
    let refused = mint_link(
        &minter,
        hop(&owner, "issues-srv").as_str(),
        &request_for(
            "issues-srv",
            LinkTarget::Upload {
                upload_id: done.session.id,
            },
        ),
    )
    .await
    .expect_err("a completed upload takes no more writes");
    assert!(
        matches!(refused, LinkMintError::UploadClosed { .. }),
        "{refused:?}"
    );
}
