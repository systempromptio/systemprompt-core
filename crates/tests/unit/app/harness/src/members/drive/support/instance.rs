use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use systemprompt_device::client::SpClient;
use systemprompt_drive::DriveClient;
use systemprompt_traits::OwnedTask;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};

use super::{StaticJwt, http};

#[derive(Debug, Clone)]
pub(crate) struct FakeRequest {
    target: String,
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

    pub(crate) fn json(status: u16, value: &Value) -> Self {
        let headers = vec![("Content-Type".to_owned(), "application/json".to_owned())];
        Self {
            status,
            headers,
            body: serde_json::to_vec(value).unwrap(),
        }
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
        let server = OwnedTask::spawn("fake-drive-instance", async move {
            let mut connections: Vec<OwnedTask<()>> = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                connections.retain(|connection| !connection.is_finished());
                let served = serve(stream, Arc::clone(&handler), Arc::clone(&seen));
                connections.push(OwnedTask::spawn("fake-drive-connection", served));
            }
        });
        Self {
            url,
            requests,
            _server: server,
        }
    }

    pub(crate) fn instance(&self) -> SpClient {
        SpClient::new(&self.url, http()).unwrap()
    }

    pub(crate) fn drive(&self) -> DriveClient {
        DriveClient::new(&self.instance(), http(), Arc::new(StaticJwt))
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
    let mut chunk = [0u8; 8192];
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
    let target = lines.next().unwrap().split(' ').nth(1).unwrap().to_owned();
    let length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .map_or(0, |(_, value)| value.trim().parse::<usize>().unwrap());
    let mut received = buffer.len() - (head_end + 4);
    while received < length {
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        received += read;
    }
    Some(FakeRequest { target })
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
