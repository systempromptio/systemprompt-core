use std::sync::{Arc, Mutex};

use rsa::RsaPrivateKey;
use rsa::pkcs1::EncodeRsaPrivateKey;
use rsa::traits::PublicKeyParts;
use systemprompt_security::keys::{Jwk, Jwks, JwksClient, RsaSigningKey};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

pub struct TestKey {
    pub kid: String,
    pub signing: RsaSigningKey,
    pub private: RsaPrivateKey,
}

impl TestKey {
    pub fn generate() -> Self {
        let signing = RsaSigningKey::generate_bits(2048).expect("generate rsa");
        let private = signing.private_key().clone();
        let kid = signing.kid().to_string();
        Self {
            kid,
            signing,
            private,
        }
    }

    pub fn jwk(&self) -> Jwk {
        Jwk::from_rsa_public_key(self.signing.public_key(), self.kid.clone())
    }

    pub fn encoding_key(&self) -> jsonwebtoken::EncodingKey {
        let der = self.private.to_pkcs1_der().expect("encode rsa der");
        jsonwebtoken::EncodingKey::from_rsa_der(der.as_bytes())
    }
}

#[derive(Clone)]
pub struct SwappableJwks {
    inner: Arc<Mutex<Jwks>>,
    fetch_count: Arc<Mutex<u32>>,
    cache_control: Arc<Mutex<Option<String>>>,
}

impl SwappableJwks {
    pub fn new(initial: Vec<Jwk>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Jwks { keys: initial })),
            fetch_count: Arc::new(Mutex::new(0)),
            cache_control: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_keys(&self, keys: Vec<Jwk>) {
        *self.inner.lock().unwrap() = Jwks { keys };
    }

    pub fn fetches(&self) -> u32 {
        *self.fetch_count.lock().unwrap()
    }

    pub fn set_cache_control(&self, header: Option<&str>) {
        *self.cache_control.lock().unwrap() = header.map(str::to_string);
    }
}

impl Respond for SwappableJwks {
    fn respond(&self, _request: &Request) -> ResponseTemplate {
        *self.fetch_count.lock().unwrap() += 1;
        let body = self.inner.lock().unwrap().clone();
        let mut template = ResponseTemplate::new(200).set_body_json(body);
        if let Some(cc) = self.cache_control.lock().unwrap().as_deref() {
            template = template.insert_header("Cache-Control", cc);
        }
        template
    }
}

pub struct JwksMock {
    pub server: MockServer,
    pub responder: SwappableJwks,
}

impl JwksMock {
    pub async fn start(initial: Vec<Jwk>) -> Self {
        let server = MockServer::start().await;
        let responder = SwappableJwks::new(initial);
        Mock::given(method("GET"))
            .and(path("/.well-known/jwks.json"))
            .respond_with(responder.clone())
            .mount(&server)
            .await;
        Self { server, responder }
    }

    pub fn issuer(&self) -> String {
        self.server.uri()
    }

    pub fn jwks_uri(&self) -> String {
        format!("{}/.well-known/jwks.json", self.server.uri())
    }
}

pub fn client_for(host: &str) -> JwksClient {
    let host = strip_scheme_port(host);
    JwksClient::new(vec![host])
}

pub fn client_for_with_min_refresh(host: &str, interval: std::time::Duration) -> JwksClient {
    let host = strip_scheme_port(host);
    JwksClient::new(vec![host]).with_min_refresh_interval(interval)
}

fn strip_scheme_port(url: &str) -> String {
    let parsed = url::Url::parse(url).expect("parse url");
    parsed.host_str().unwrap_or("").to_string()
}

#[allow(dead_code)]
pub fn rsa_n_e(key: &RsaPrivateKey) -> (Vec<u8>, Vec<u8>) {
    (
        key.to_public_key().n().to_bytes_be(),
        key.to_public_key().e().to_bytes_be(),
    )
}
