use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use systemprompt_device::accounts::{Accounts, AccountsConfig, Browser, BrowserError};
use systemprompt_device::client::HttpClient;
use systemprompt_device::protocol::ids::InstallId;
use systemprompt_device::secure::{MemorySealer, Sealer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use url::Url;

pub(crate) const TOKEN: &str = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1MSIsImV4cCI6MTg5MzQ1NjAwMCwic2Vzc2lvbl9pZCI6InNlc3MtMSJ9.c2ln";
pub(crate) const PAT: &str = "sp-live-test-token";
pub(crate) const CODE: &str = "abc123def456";
const PUBKEY: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";

struct Request {
    method: String,
    path: String,
    authorization: Option<String>,
    body: String,
}

pub(crate) struct FakeInstance {
    pub(crate) url: String,
    pub(crate) refuse_pat: Arc<AtomicBool>,
    task: JoinHandle<()>,
}

impl Drop for FakeInstance {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeInstance {
    pub(crate) async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let refuse_pat = Arc::new(AtomicBool::new(false));
        let refusing = Arc::clone(&refuse_pat);
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let refusing = Arc::clone(&refusing);
                tokio::spawn(async move { serve(stream, &refusing).await });
            }
        });
        Self {
            url,
            refuse_pat,
            task,
        }
    }
}

async fn serve(mut stream: TcpStream, refuse_pat: &AtomicBool) {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break at;
        }
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            return;
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8(buffer[..head_end].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap().split(' ');
    let method = first.next().unwrap().to_owned();
    let target = first.next().unwrap().to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .map_or(0, |(_, value)| value.parse().unwrap());
    let mut body = buffer[head_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    let request = Request {
        method,
        path: target.split('?').next().unwrap().to_owned(),
        authorization: headers
            .iter()
            .find(|(name, _)| name == "authorization")
            .map(|(_, value)| value.clone()),
        body: String::from_utf8(body).unwrap(),
    };
    let (status, json) = answer(&request, refuse_pat.load(Ordering::SeqCst));
    let text = json.to_string();
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
    stream.write_all(response.as_bytes()).await.unwrap();
    stream.shutdown().await.ok();
}

fn answer(request: &Request, refuse_pat: bool) -> (u16, serde_json::Value) {
    let bearer =
        |value: &str| request.authorization.as_deref() == Some(&*format!("Bearer {value}"));
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/v1/auth/bridge/capabilities") => {
            (200, serde_json::json!({ "modes": ["pat", "session"] }))
        },
        ("GET", "/api/v1/health") => (
            200,
            serde_json::json!({ "status": "ok", "version": "0.90.0" }),
        ),
        ("POST", "/v1/auth/bridge/session-pat") if request.body.contains(CODE) => {
            (200, serde_json::json!({ "pat": PAT }))
        },
        ("POST", "/v1/auth/bridge/pat") if bearer(PAT) && !refuse_pat => (
            200,
            serde_json::json!({ "token": TOKEN, "ttl": 3600, "headers": { "x-session-id": "sess-1" } }),
        ),
        ("GET", "/v1/bridge/whoami") if bearer(TOKEN) => (
            200,
            serde_json::json!({ "user_id": "user-1", "email": "dev@example.test", "roles": ["user"] }),
        ),
        ("GET", "/v1/bridge/pubkey") => (200, serde_json::json!({ "pubkey": PUBKEY })),
        ("GET", "/v1/bridge/manifest") => (503, serde_json::json!({ "message": "down" })),
        ("POST", _) => (
            401,
            serde_json::json!({ "error_key": "invalid_credential" }),
        ),
        _ => (404, serde_json::json!({})),
    }
}

#[derive(Debug, Default)]
pub(crate) struct FakeBrowser {
    pub(crate) opened: Mutex<Vec<Url>>,
    pub(crate) answer_with: Option<&'static str>,
}

impl Browser for FakeBrowser {
    fn open(&self, url: &Url) -> Result<(), BrowserError> {
        self.opened.lock().unwrap().push(url.clone());
        let Some(code) = self.answer_with else {
            return Ok(());
        };
        let redirect = url
            .query_pairs()
            .find(|(key, _)| key == "redirect")
            .map(|(_, value)| value.into_owned());
        let Some(redirect) = redirect else {
            return Ok(());
        };
        let redirect = Url::parse(&redirect).unwrap();
        let port = redirect.port().unwrap();
        let target = format!(
            "{}?{}&code={code}",
            redirect.path(),
            redirect.query().unwrap_or("")
        );
        tokio::spawn(async move {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let request = format!("GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
            stream.write_all(request.as_bytes()).await.unwrap();
            let mut answer = Vec::new();
            stream.read_to_end(&mut answer).await.ok();
        });
        Ok(())
    }
}

pub(crate) fn config(
    path: &Path,
    sealer: Arc<dyn Sealer>,
    browser: Arc<FakeBrowser>,
) -> AccountsConfig {
    AccountsConfig {
        http: HttpClient::new("harness-tests/0").unwrap(),
        sealer,
        state_file: path.to_owned(),
        product: "Harness Tests".to_owned(),
        install_id: InstallId::new("install-1"),
        browser,
        device_name: "Harness Tests on WS-1".to_owned(),
    }
}

pub(crate) fn open(path: &Path, sealer: Arc<dyn Sealer>, browser: Arc<FakeBrowser>) -> Accounts {
    Accounts::open(config(path, sealer, browser)).unwrap()
}

pub(crate) fn open_plain(path: &Path) -> Accounts {
    open(path, Arc::new(MemorySealer::for_tests()), Arc::default())
}

pub(crate) fn store_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("state").join("accounts.json")
}

pub(crate) fn write_store(path: &Path, store: &serde_json::Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec_pretty(store).unwrap()).unwrap();
}

pub(crate) fn account(id: &str, label: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "label": label,
        "url": format!("https://{id}.example.com"),
        "added_at": "2030-03-17T17:46:40Z",
    })
}
