use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_config::{SecretsBootstrapError, SecretsProvider, VaultError, VaultKvProvider};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::vault_fixture as fx;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/vault_tls")
        .join(name)
}

fn acceptor() -> TlsAcceptor {
    let certs = CertificateDer::pem_file_iter(fixture("leaf.pem"))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let key = PrivateKeyDer::from_pem_file(fixture("leaf_key.pem")).unwrap();
    let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();
    TlsAcceptor::from(Arc::new(config))
}

async fn serve_one_kv_read(listener: TcpListener) -> bool {
    let (socket, _) = listener.accept().await.unwrap();
    let Ok(mut tls) = acceptor().accept(socket).await else {
        return false;
    };
    let mut request = [0u8; 4096];
    let read = tls.read(&mut request).await.unwrap();
    assert!(read > 0, "the client sent a request over TLS");
    let body = fx::kv_body(fx::secrets_document()).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    tls.write_all(response.as_bytes()).await.unwrap();
    tls.shutdown().await.unwrap();
    true
}

async fn https_listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("https://{}", listener.local_addr().unwrap());
    (listener, address)
}

#[tokio::test]
async fn a_vault_signed_by_the_configured_ca_is_trusted() {
    let (listener, address) = https_listener().await;
    let server = tokio::spawn(serve_one_kv_read(listener));
    let mut cfg = fx::config(&address, fx::token_auth());
    cfg.retries = 0;
    cfg.ca_cert_path = Some(fixture("ca.pem").display().to_string());

    let document = VaultKvProvider::from_config(&cfg, |_name| Some("root-token".to_owned()))
        .unwrap()
        .fetch()
        .await
        .unwrap();

    assert!(server.await.unwrap(), "the TLS handshake completed");
    assert!(document.key_names().contains(&"database_url".to_owned()));
}

#[tokio::test]
async fn a_vault_signed_by_an_unconfigured_ca_is_refused() {
    let (listener, address) = https_listener().await;
    let server = tokio::spawn(serve_one_kv_read(listener));
    let mut cfg = fx::config(&address, fx::token_auth());
    cfg.retries = 0;

    let err = VaultKvProvider::from_config(&cfg, |_name| Some("root-token".to_owned()))
        .unwrap()
        .fetch()
        .await
        .unwrap_err();

    assert!(!server.await.unwrap(), "the TLS handshake was rejected");
    assert!(
        matches!(
            err,
            SecretsBootstrapError::Vault(VaultError::Exhausted { .. })
        ),
        "got {err}"
    );
}
