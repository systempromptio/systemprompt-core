use crate::error::{ClientError, ClientResult};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use systemprompt_identifiers::JwtToken;

async fn extract_error(response: Response) -> ClientError {
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, status = %status, "Failed to read error response body");
        format!("(body unreadable: {})", e)
    });
    ClientError::from_response(status, body)
}

fn apply_auth(
    request: reqwest::RequestBuilder,
    token: Option<&JwtToken>,
) -> reqwest::RequestBuilder {
    match token {
        Some(t) => request.header("Authorization", format!("Bearer {}", t.as_str())),
        None => request,
    }
}

async fn send_checked(request: reqwest::RequestBuilder) -> ClientResult<Response> {
    let response = request.send().await?;
    if response.status().is_success() {
        Ok(response)
    } else {
        Err(extract_error(response).await)
    }
}

pub(crate) async fn get<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    token: Option<&JwtToken>,
) -> ClientResult<T> {
    let response = send_checked(apply_auth(client.get(url), token)).await?;
    Ok(response.json().await?)
}

pub(crate) async fn post<T: DeserializeOwned, B: serde::Serialize + Sync>(
    client: &Client,
    url: &str,
    body: &B,
    token: Option<&JwtToken>,
) -> ClientResult<T> {
    let response = send_checked(apply_auth(client.post(url), token).json(body)).await?;
    Ok(response.json().await?)
}

pub(crate) async fn put<B: serde::Serialize + Sync>(
    client: &Client,
    url: &str,
    body: &B,
    token: Option<&JwtToken>,
) -> ClientResult<()> {
    send_checked(apply_auth(client.put(url), token).json(body)).await?;
    Ok(())
}

pub(crate) async fn delete(client: &Client, url: &str, token: Option<&JwtToken>) -> ClientResult<()> {
    send_checked(apply_auth(client.delete(url), token)).await?;
    Ok(())
}
