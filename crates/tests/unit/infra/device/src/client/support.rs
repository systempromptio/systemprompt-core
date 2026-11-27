use std::sync::{Arc, Mutex};

use systemprompt_device::client::{HttpClient, SpClient};
use systemprompt_device::protocol::Jwt;
use systemprompt_traits::OwnedTask;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub(crate) const USER_AGENT: &str = "harness-tests/0.0.0";
pub(crate) const CODE: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

pub(crate) fn jwt() -> Jwt {
    Jwt::new("header.claims.signature").unwrap()
}

pub(crate) fn http() -> HttpClient {
    HttpClient::new(USER_AGENT).unwrap()
}

pub(crate) async fn raw_request(port: u16, method: &str, target: &str) -> String {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let request = format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.unwrap();
    answer
}

pub(crate) async fn raw_get(port: u16, target: &str) -> String {
    raw_request(port, "GET", target).await
}

#[derive(Debug, Clone)]
pub(crate) struct FakeRequest {
    pub(crate) method: String,
    pub(crate) target: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl FakeRequest {
    pub(crate) fn path(&self) -> &str {
        self.target
            .split_once('?')
            .map_or(self.target.as_str(), |(path, _)| path)
    }

    pub(crate) fn query(&self) -> &str {
        self.target.split_once('?').map_or("", |(_, query)| query)
    }

    pub(crate) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub(crate) fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).unwrap()
    }

    pub(crate) fn text(&self) -> String {
        String::from_utf8(self.body.clone()).unwrap()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FakeResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl FakeResponse {
    pub(crate) fn status(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub(crate) fn json(status: u16, value: &serde_json::Value) -> Self {
        Self {
            status,
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            body: serde_json::to_vec(value).unwrap(),
        }
    }

    pub(crate) fn text(status: u16, body: &str) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    pub(crate) fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }
}

type Handler = Arc<dyn Fn(&FakeRequest) -> FakeResponse + Send + Sync>;
type Seen = Arc<Mutex<Vec<FakeRequest>>>;

pub(crate) struct FakeInstance {
    pub(crate) url: String,
    requests: Seen,
    _server: OwnedTask<()>,
}

impl FakeInstance {
    pub(crate) async fn start<H>(handler: H) -> Self
    where
        H: Fn(&FakeRequest) -> FakeResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let requests: Seen = Arc::new(Mutex::new(Vec::new()));
        let handler: Handler = Arc::new(handler);
        let seen = Arc::clone(&requests);
        let server = OwnedTask::spawn("fake-instance", async move {
            let mut connections: Vec<OwnedTask<()>> = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                connections.retain(|connection| !connection.is_finished());
                let served = serve(stream, Arc::clone(&handler), Arc::clone(&seen));
                connections.push(OwnedTask::spawn("fake-connection", served));
            }
        });
        Self {
            url,
            requests,
            _server: server,
        }
    }

    pub(crate) fn client(&self) -> SpClient {
        SpClient::new(&self.url, http()).unwrap()
    }

    pub(crate) fn requests(&self) -> Vec<FakeRequest> {
        self.requests.lock().unwrap().clone()
    }

    pub(crate) fn last(&self) -> FakeRequest {
        self.requests().pop().unwrap()
    }
}

async fn read_request(stream: &mut TcpStream) -> Option<FakeRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let head_end = loop {
        if let Some(at) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
            break at;
        }
        let read = stream.read(&mut chunk).await.unwrap();
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
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Some(FakeRequest {
        method,
        target,
        headers,
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
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        response.body.len()
    ));
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(&response.body);
    stream.write_all(&bytes).await.unwrap();
    stream.flush().await.unwrap();
    stream.shutdown().await.ok();
}
