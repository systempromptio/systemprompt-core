use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use systemprompt_traits::OwnedTask;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug, Clone)]
pub(crate) struct FakeRequest {
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) body: Vec<u8>,
}

impl FakeRequest {
    pub(crate) fn path(&self) -> &str {
        self.target
            .split_once('?')
            .map_or(self.target.as_str(), |(path, _)| path)
    }

    pub(crate) fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FakeResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl FakeResponse {
    pub(crate) fn json(status: u16, value: &Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            body: serde_json::to_vec(value).unwrap(),
        }
    }

    pub(crate) fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }
}

type Handler = Arc<dyn Fn(&FakeRequest) -> FakeResponse + Send + Sync>;
type Seen = Arc<Mutex<Vec<FakeRequest>>>;

pub(crate) struct FakeInstance {
    pub(crate) url: String,
    _server: OwnedTask<()>,
}

impl FakeInstance {
    pub(crate) async fn start<H>(handler: H) -> Self
    where
        H: Fn(&FakeRequest) -> FakeResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let seen: Seen = Arc::new(Mutex::new(Vec::new()));
        let handler: Handler = Arc::new(handler);
        let server = OwnedTask::spawn("fake-notifications-instance", async move {
            let mut connections: Vec<OwnedTask<()>> = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                connections.retain(|connection| !connection.is_finished());
                let served = serve(stream, Arc::clone(&handler), Arc::clone(&seen));
                connections.push(OwnedTask::spawn("fake-notifications-connection", served));
            }
        });
        Self {
            url,
            _server: server,
        }
    }
}

async fn read_request(stream: &mut TcpStream) -> Option<FakeRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break at;
        }
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    };
    let head = String::from_utf8(buffer[..head_end].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next().unwrap().split(' ');
    let method = request_line.next().unwrap().to_owned();
    let target = request_line.next().unwrap().to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, value)| value.parse::<usize>().unwrap());
    let mut body = buffer[head_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Some(FakeRequest {
        method,
        target,
        body,
    })
}

async fn serve(mut stream: TcpStream, handler: Handler, seen: Seen) {
    let Some(request) = read_request(&mut stream).await else {
        return;
    };
    let response = handler(&request);
    seen.lock().unwrap().push(request);
    let mut out = format!("HTTP/1.1 {} Fake\r\n", response.status);
    for (name, value) in &response.headers {
        write!(out, "{name}: {value}\r\n").unwrap();
    }
    write!(
        out,
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.body.len()
    )
    .unwrap();
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(&response.body);
    stream.write_all(&bytes).await.unwrap();
    stream.flush().await.unwrap();
    stream.shutdown().await.ok();
}
