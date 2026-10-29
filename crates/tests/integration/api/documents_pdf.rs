//! `GET /api/v1/documents/{id}/content?format=pdf`: the revision rendered
//! as PDF, with a session and with a signed download link.

use anyhow::Result;
use axum::body::Body;
use axum::http::{Method, StatusCode, header};
use systemprompt_api::routes::documents::{router, with_download_tokens};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_runtime::documents::download::{
    DOWNLOAD_AUDIENCE, DOWNLOAD_TTL, download_purpose,
};
use systemprompt_test_fixtures::{parse_pdf, seed_bridge_credential};

use super::documents::{
    app, create_markdown, fixture, head_of, id_of, mint_download, put_content, request, send,
};

fn header_of(reply: &super::documents::Reply, name: header::HeaderName) -> String {
    reply
        .headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

#[tokio::test]
async fn a_pdf_read_serves_the_rendered_revision_and_is_deterministic() -> Result<()> {
    let (pool, _ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-pdf").await?;
    let created = create_markdown(&state, &owner).await?;
    let (id, base) = (id_of(&created), head_of(&created));
    let written = put_content(
        &state,
        &owner,
        &id,
        &base,
        "# Launch plan\n\nThe launch is in March.\n\n- first task\n",
    )
    .await?;
    assert_eq!(written.status, StatusCode::OK, "{}", written.json());
    let revision = written.json()["revision"]["id"]
        .as_str()
        .expect("revision")
        .to_owned();

    let get = |uri: String| {
        send(
            app(&state, &owner),
            request(Method::GET, &uri)
                .body(Body::empty())
                .expect("request"),
        )
    };
    let pdf = get(format!("/{id}/content?format=pdf")).await?;
    assert_eq!(pdf.status, StatusCode::OK, "{}", pdf.json());
    assert_eq!(header_of(&pdf, header::CONTENT_TYPE), "application/pdf");
    assert!(header_of(&pdf, header::CONTENT_DISPOSITION).contains(".pdf"));
    assert_eq!(
        header_of(&pdf, header::HeaderName::from_static("x-revision-id")),
        revision
    );
    assert!(pdf.body.starts_with(b"%PDF-"), "not a pdf");
    let text = parse_pdf(&pdf.body).text();
    assert!(
        text.contains("Launch plan") && text.contains("first task"),
        "{text}"
    );

    let again = get(format!("/{id}/content?format=pdf&revision={revision}")).await?;
    assert_eq!(again.status, StatusCode::OK);
    assert_eq!(again.body, pdf.body, "two renders of one revision differ");
    assert_eq!(
        header_of(&again, header::ETAG),
        header_of(&pdf, header::ETAG)
    );

    let head = send(
        app(&state, &owner),
        request(Method::HEAD, &format!("/{id}/content?format=pdf"))
            .body(Body::empty())
            .expect("request"),
    )
    .await?;
    assert_eq!(head.status, StatusCode::OK);
    assert!(head.body.is_empty());
    assert_eq!(
        header_of(&head, header::CONTENT_LENGTH),
        pdf.body.len().to_string()
    );

    let unknown = get(format!("/{id}/content?format=rtf")).await?;
    assert_eq!(
        unknown.status,
        StatusCode::BAD_REQUEST,
        "{}",
        unknown.json()
    );
    Ok(())
}

#[tokio::test]
async fn a_signed_download_link_serves_the_pdf_rendition() -> Result<()> {
    let (pool, ctx, state) = fixture().await?;
    let owner = seed_bridge_credential(&pool, "docs-pdf-link").await?;
    let created = create_markdown(&state, &owner).await?;
    let (id, revision) = (id_of(&created), head_of(&created));
    let token = mint_download(
        ctx.token_authority().as_ref(),
        &owner,
        DOWNLOAD_AUDIENCE,
        DOWNLOAD_TTL,
        download_purpose(&DocumentId::new(&id), &DocumentRevisionId::new(&revision)),
    );
    let tokenised = with_download_tokens(router(state.clone()), state.clone());
    let linked = send(
        tokenised,
        request(
            Method::GET,
            &format!("/{id}/content?revision={revision}&format=pdf&access_token={token}"),
        )
        .body(Body::empty())
        .expect("request"),
    )
    .await?;
    assert_eq!(linked.status, StatusCode::OK, "{}", linked.json());
    assert_eq!(header_of(&linked, header::CONTENT_TYPE), "application/pdf");
    assert!(linked.body.starts_with(b"%PDF-"));
    let text = parse_pdf(&linked.body).text();
    assert!(text.contains("Plan"), "{text}");
    Ok(())
}
