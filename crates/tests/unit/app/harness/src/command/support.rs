use std::path::Path;
use std::sync::{Arc, Mutex};

use systemprompt_device::secure::MemorySealer;
use systemprompt_harness::command::{HeadlessHost, MemorySettings};
use systemprompt_harness::extension::HostInfo;
use systemprompt_harness::members::REGISTERED;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

pub(crate) const TOKEN: &str = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1MSIsImV4cCI6MTg5MzQ1NjAwMCwic2Vzc2lvbl9pZCI6InNlc3MtMSJ9.c2ln";
pub(crate) const PAT: &str = "sp-live-test-token";
pub(crate) const FIRST_KEY: [u8; 32] = [7u8; 32];
pub(crate) const SECOND_KEY: [u8; 32] = [9u8; 32];
const FIRST_KEY_BASE64: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc=";
const SECOND_KEY_BASE64: &str = "CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=";

pub(crate) fn host(dir: &Path) -> HeadlessHost {
    HeadlessHost {
        info: HostInfo {
            product: "harness-tests",
            display_name: "Harness Tests",
            version: "0.0.0",
        },
        project_dir: ".harness".to_owned(),
        device_name: "Harness Tests on WS-1".to_owned(),
        data_root: dir.join("root"),
        home: dir.join("home"),
        documents: None,
        sealer: Arc::new(MemorySealer::for_tests()),
        settings: Arc::new(MemorySettings::default()),
        registrations: REGISTERED.to_vec(),
        verdicts_enabled: true,
        search_path: None,
        path_ext: None,
    }
}

#[derive(Debug, Default)]
struct Behaviour {
    refuse_pat: bool,
    second_key: bool,
    seen: Vec<String>,
}

pub(crate) struct FakeInstance {
    pub(crate) url: String,
    behaviour: Arc<Mutex<Behaviour>>,
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
        let behaviour = Arc::new(Mutex::new(Behaviour::default()));
        let shared = Arc::clone(&behaviour);
        let task = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let shared = Arc::clone(&shared);
                tokio::spawn(async move { serve(stream, &shared).await });
            }
        });
        Self {
            url,
            behaviour,
            task,
        }
    }

    pub(crate) fn refuse_pat(&self) {
        self.behaviour.lock().unwrap().refuse_pat = true;
    }

    pub(crate) fn serve_second_key(&self) {
        self.behaviour.lock().unwrap().second_key = true;
    }

    pub(crate) fn seen(&self) -> Vec<String> {
        self.behaviour.lock().unwrap().seen.clone()
    }
}

pub(crate) async fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

async fn serve(mut stream: TcpStream, behaviour: &Mutex<Behaviour>) {
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
    let path = target.split('?').next().unwrap().to_owned();
    let authorization = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.trim().to_owned());
    let (status, json) = {
        let mut behaviour = behaviour.lock().unwrap();
        behaviour.seen.push(format!("{method} {path}"));
        answer(&method, &path, authorization.as_deref(), &behaviour)
    };
    let text = json.to_string();
    let response = format!(
        "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
    stream.write_all(response.as_bytes()).await.unwrap();
    stream.shutdown().await.ok();
}

fn answer(
    method: &str,
    path: &str,
    authorization: Option<&str>,
    behaviour: &Behaviour,
) -> (u16, serde_json::Value) {
    let bearer = |value: &str| authorization == Some(&*format!("Bearer {value}"));
    match (method, path) {
        ("POST", "/v1/auth/bridge/pat") if bearer(PAT) && !behaviour.refuse_pat => (
            200,
            serde_json::json!({ "token": TOKEN, "ttl": 3600, "headers": { "x-session-id": "sess-1" } }),
        ),
        ("GET", "/v1/bridge/whoami") if bearer(TOKEN) => (
            200,
            serde_json::json!({ "user_id": "user-1", "email": "dev@example.test", "roles": ["user"] }),
        ),
        ("GET", "/v1/bridge/pubkey") => {
            let key = if behaviour.second_key {
                SECOND_KEY_BASE64
            } else {
                FIRST_KEY_BASE64
            };
            (200, serde_json::json!({ "pubkey": key }))
        },
        ("GET", "/v1/bridge/manifest") => (503, serde_json::json!({ "message": "down" })),
        ("POST", _) => (
            401,
            serde_json::json!({ "error_key": "invalid_credential" }),
        ),
        _ => (404, serde_json::json!({})),
    }
}
