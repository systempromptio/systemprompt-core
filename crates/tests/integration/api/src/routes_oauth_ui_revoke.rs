//! `/oauth/revoke` for a person's UI session over the real router: a `ui`
//! bearer ends its own access token or its own refresh token (and with the
//! refresh token the bearer it presented), it reaches no token of another
//! client or another user, and a first-party caller revoking a `ui` access
//! token really ends it.

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};

use super::routes_oauth_person_ui::{
    Person, person, refresh, same_person_through_another_client, send, sign_in,
};

struct Grant {
    access: String,
    refresh: String,
}

async fn grant(p: &Person) -> anyhow::Result<Grant> {
    let (status, v) = sign_in(p, 5).await?;
    assert_eq!(status, StatusCode::OK, "{v}");
    Ok(Grant {
        access: v["access_token"].as_str().unwrap_or_default().to_owned(),
        refresh: v["refresh_token"].as_str().unwrap_or_default().to_owned(),
    })
}

async fn revoke(p: &Person, bearer: &str, token: &str, hint: Option<&str>) -> anyhow::Result<()> {
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("token", token);
    if let Some(hint) = hint {
        form.append_pair("token_type_hint", hint);
    }
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/core/oauth/revoke")
        .header(header::HOST, "127.0.0.1")
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(form.finish()))?;
    let (status, text) = send(&p.app, req).await?;
    assert_eq!(status, StatusCode::OK, "{text}");
    Ok(())
}

async fn documents(p: &Person, token: &str) -> anyhow::Result<StatusCode> {
    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/documents")
        .header(header::HOST, "127.0.0.1")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())?;
    Ok(send(&p.app, req).await?.0)
}

#[tokio::test]
async fn a_ui_bearer_revokes_its_own_access_token() -> anyhow::Result<()> {
    let p = person(true).await?;
    let g = grant(&p).await?;
    assert_eq!(documents(&p, &g.access).await?, StatusCode::OK);
    revoke(&p, &g.access, &g.access, Some("access_token")).await?;
    assert_eq!(documents(&p, &g.access).await?, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn a_ui_bearer_revokes_its_own_refresh_token_and_its_session_ends() -> anyhow::Result<()> {
    let p = person(true).await?;
    let g = grant(&p).await?;
    assert_eq!(documents(&p, &g.access).await?, StatusCode::OK);
    revoke(&p, &g.access, &g.refresh, Some("refresh_token")).await?;
    let (status, v) = refresh(&p, &g.refresh).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert_eq!(documents(&p, &g.access).await?, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn a_ui_bearer_cannot_revoke_a_token_of_another_client() -> anyhow::Result<()> {
    let p = person(true).await?;
    let own = grant(&p).await?;
    let other = same_person_through_another_client(&p, true).await?;
    let theirs = grant(&other).await?;
    revoke(&p, &own.access, &theirs.access, Some("access_token")).await?;
    revoke(&p, &own.access, &theirs.refresh, Some("refresh_token")).await?;
    revoke(&p, &own.access, &theirs.access, None).await?;
    revoke(&p, &own.access, &theirs.refresh, None).await?;
    assert_eq!(documents(&other, &theirs.access).await?, StatusCode::OK);
    assert_eq!(documents(&p, &own.access).await?, StatusCode::OK);
    let (status, v) = refresh(&other, &theirs.refresh).await?;
    assert_eq!(status, StatusCode::OK, "{v}");
    Ok(())
}

#[tokio::test]
async fn a_ui_bearer_cannot_revoke_a_token_of_another_user() -> anyhow::Result<()> {
    let p = person(true).await?;
    let own = grant(&p).await?;
    let stranger = person(true).await?;
    let theirs = grant(&stranger).await?;
    revoke(&p, &own.access, &theirs.access, Some("access_token")).await?;
    revoke(&p, &own.access, &theirs.refresh, Some("refresh_token")).await?;
    assert_eq!(documents(&stranger, &theirs.access).await?, StatusCode::OK);
    assert_eq!(documents(&p, &own.access).await?, StatusCode::OK);
    let (status, v) = refresh(&stranger, &theirs.refresh).await?;
    assert_eq!(status, StatusCode::OK, "{v}");
    Ok(())
}

#[tokio::test]
async fn a_first_party_caller_revoking_a_ui_access_token_ends_it() -> anyhow::Result<()> {
    let p = person(true).await?;
    let ui = grant(&p).await?;
    let web = same_person_through_another_client(&p, false).await?;
    let first_party = grant(&web).await?;
    assert_eq!(documents(&p, &ui.access).await?, StatusCode::OK);
    revoke(&web, &first_party.access, &ui.access, Some("access_token")).await?;
    assert_eq!(documents(&p, &ui.access).await?, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn revoke_is_the_only_oauth_route_that_takes_a_ui_bearer() -> anyhow::Result<()> {
    let p = person(true).await?;
    let g = grant(&p).await?;
    for (method, uri) in [
        (Method::GET, "/api/v1/core/oauth/userinfo"),
        (Method::POST, "/api/v1/core/oauth/introspect"),
        (Method::POST, "/api/v1/core/oauth/logout"),
        (Method::GET, "/api/v1/core/oauth/clients"),
    ] {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, "127.0.0.1")
            .header(header::AUTHORIZATION, format!("Bearer {}", g.access))
            .body(Body::empty())?;
        let (status, text) = send(&p.app, req).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}: {text}");
    }
    Ok(())
}
