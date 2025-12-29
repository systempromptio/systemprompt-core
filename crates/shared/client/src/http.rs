use crate::error::{ClientError, ClientResult};
use reqwest::Client;
use serde::de::DeserializeOwned;
use systemprompt_identifiers::JwtToken;

pub async fn get<T: DeserializeOwned>(
    client: &Client,
    url: &str,
    token: Option<&JwtToken>,
) -> ClientResult<T> {
    let mut request = client.get(url);

    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {}", token.as_str()));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!(error = %e, status = %status, "Failed to read error response body");
                format!("(body unreadable: {})", e)
            },
        };
        return Err(ClientError::from_response(status, body));
    }

    let data: T = response.json().await?;
    Ok(data)
}

pub async fn post<T: DeserializeOwned, B: serde::Serialize + Sync>(
    client: &Client,
    url: &str,
    body: &B,
    token: Option<&JwtToken>,
) -> ClientResult<T> {
    let mut request = client.post(url).header("Content-Type", "application/json");

    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {}", token.as_str()));
    }

    let response = request.json(body).send().await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!(error = %e, status = %status, "Failed to read error response body");
                format!("(body unreadable: {})", e)
            },
        };
        return Err(ClientError::from_response(status, body));
    }

    let data: T = response.json().await?;
    Ok(data)
}

pub async fn put<B: serde::Serialize + Sync>(
    client: &Client,
    url: &str,
    body: &B,
    token: Option<&JwtToken>,
) -> ClientResult<()> {
    let mut request = client.put(url).header("Content-Type", "application/json");

    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {}", token.as_str()));
    }

    let response = request.json(body).send().await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!(error = %e, status = %status, "Failed to read error response body");
                format!("(body unreadable: {})", e)
            },
        };
        return Err(ClientError::from_response(status, body));
    }

    Ok(())
}

pub async fn delete(client: &Client, url: &str, token: Option<&JwtToken>) -> ClientResult<()> {
    let mut request = client.delete(url);

    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {}", token.as_str()));
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = match response.text().await {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!(error = %e, status = %status, "Failed to read error response body");
                format!("(body unreadable: {})", e)
            },
        };
        return Err(ClientError::from_response(status, body));
    }

    Ok(())
}
