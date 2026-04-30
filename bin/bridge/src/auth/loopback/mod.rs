use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};
use systemprompt_identifiers::ValidatedUrl;

pub const LOOPBACK_PORT: u16 = 8767;
pub const LOOPBACK_TIMEOUT_SECS: u64 = 300;

const SUCCESS_HTML: &str = include_str!("success.html");
const ERROR_HTML: &str = include_str!("error.html");

#[derive(Debug, thiserror::Error)]
pub enum LoopbackError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bind 127.0.0.1:{port} failed: {source}")]
    Bind {
        port: u16,
        #[source]
        source: std::io::Error,
    },
    #[error("timed out after {0}s waiting for browser callback")]
    Timeout(u64),
    #[error("unexpected method {0}")]
    UnexpectedMethod(String),
    #[error("dashboard reported error: {0}")]
    DashboardError(String),
    #[error("callback missing ?code=... parameter")]
    MissingCode,
}

pub type Result<T> = std::result::Result<T, LoopbackError>;

pub struct Captured {
    pub code: String,
}

pub struct LoopbackServer {
    listener: TcpListener,
    addr: SocketAddr,
}

impl LoopbackServer {
    pub fn bind() -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", LOOPBACK_PORT)).map_err(|source| {
            LoopbackError::Bind {
                port: LOOPBACK_PORT,
                source,
            }
        })?;
        let addr = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, addr })
    }

    #[must_use]
    pub fn callback_url(&self) -> ValidatedUrl {
        let raw = format!("http://127.0.0.1:{}/callback", self.addr.port());
        ValidatedUrl::try_new(raw).unwrap_or_else(|_| {
            ValidatedUrl::try_new("http://127.0.0.1/callback").unwrap_or_else(|_| unreachable_url())
        })
    }

    pub fn accept_callback(self, timeout: Duration) -> Result<Captured> {
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() >= deadline {
                return Err(LoopbackError::Timeout(timeout.as_secs()));
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    return handle_connection(stream);
                },
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(100));
                },
                Err(e) => return Err(e.into()),
            }
        }
    }
}

fn unreachable_url() -> ValidatedUrl {
    ValidatedUrl::try_new("http://127.0.0.1/").unwrap_or_else(|_| unreachable_url())
}

fn handle_connection(mut stream: TcpStream) -> Result<Captured> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let request_line = read_request_line(&mut reader)?;
    drain_headers(&mut reader)?;

    let outcome = parse_code(&request_line);
    let (status, body) = match &outcome {
        Ok(_) => ("200 OK", SUCCESS_HTML),
        Err(_) => ("400 Bad Request", ERROR_HTML),
    };
    write_response(&mut stream, status, body)?;
    outcome.map(|code| Captured { code })
}

fn read_request_line<R: BufRead>(reader: &mut R) -> Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}

fn drain_headers<R: BufRead>(reader: &mut R) -> Result<()> {
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 || line == "\r\n" || line == "\n" {
            return Ok(());
        }
    }
}

fn parse_code(request_line: &str) -> Result<String> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    if method != "GET" {
        return Err(LoopbackError::UnexpectedMethod(method.to_string()));
    }
    let query = target.split_once('?').map_or("", |(_, q)| q);
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("code=") {
            let decoded = url_decode(value);
            if !decoded.is_empty() {
                return Ok(decoded);
            }
        }
        if let Some(err) = pair.strip_prefix("error=") {
            return Err(LoopbackError::DashboardError(url_decode(err)));
        }
    }
    Err(LoopbackError::MissingCode)
}

fn url_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            },
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_nibble(bytes[i + 1]);
                let lo = hex_nibble(bytes[i + 2]);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push((h << 4) | l);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b => {
                out.push(b);
                i += 1;
            },
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn write_response(stream: &mut TcpStream, status: &str, body: &str) -> Result<()> {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: \
         {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    let _ = stream.read(&mut [0u8; 16]);
    Ok(())
}
