use std::io::{BufRead, BufReader, Read};
use std::net::TcpStream;

pub const MAX_HEADER_BYTES: usize = 32 * 1024;
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn target(&self) -> String {
        if self.query.is_empty() {
            self.path.clone()
        } else {
            format!("{}?{}", self.path, self.query)
        }
    }
}

pub fn parse(stream: &mut TcpStream) -> Result<Request, String> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|e| format!("read request line: {e}"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "missing method".to_string())?
        .to_string();
    let raw_target = parts
        .next()
        .ok_or_else(|| "missing target".to_string())?
        .to_string();

    let (path, query) = match raw_target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (raw_target, String::new()),
    };

    let mut headers: Vec<(String, String)> = Vec::new();
    let mut total = request_line.len();
    let mut content_length = 0usize;
    let mut transfer_encoding: Option<String> = None;
    loop {
        let mut header = String::new();
        reader
            .read_line(&mut header)
            .map_err(|e| format!("read header: {e}"))?;
        total += header.len();
        if total > MAX_HEADER_BYTES {
            return Err("headers too large".into());
        }
        let trimmed = header.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let (name, value) = match trimmed.split_once(':') {
            Some((n, v)) => (n.trim().to_string(), v.trim().to_string()),
            None => continue,
        };
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value.parse::<usize>().map_err(|e| e.to_string())?;
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            transfer_encoding = Some(value.to_ascii_lowercase());
        }
        headers.push((name, value));
    }

    if content_length > MAX_BODY_BYTES {
        return Err(format!("body too large: {content_length}"));
    }

    let mut body = Vec::new();
    if let Some(te) = transfer_encoding.as_deref() {
        if te.contains("chunked") {
            read_chunked(&mut reader, &mut body)?;
        } else if content_length > 0 {
            body.resize(content_length, 0);
            reader
                .read_exact(&mut body)
                .map_err(|e| format!("read body: {e}"))?;
        }
    } else if content_length > 0 {
        body.resize(content_length, 0);
        reader
            .read_exact(&mut body)
            .map_err(|e| format!("read body: {e}"))?;
    }

    Ok(Request {
        method,
        path,
        query,
        headers,
        body,
    })
}

fn read_chunked<R: BufRead>(reader: &mut R, out: &mut Vec<u8>) -> Result<(), String> {
    loop {
        let mut size_line = String::new();
        reader
            .read_line(&mut size_line)
            .map_err(|e| format!("read chunk size: {e}"))?;
        let size_str = size_line.trim_end_matches(['\r', '\n']);
        let size_str = size_str.split(';').next().unwrap_or("0").trim();
        let size =
            usize::from_str_radix(size_str, 16).map_err(|e| format!("chunk size parse: {e}"))?;
        if out.len() + size > MAX_BODY_BYTES {
            return Err("chunked body too large".into());
        }
        if size == 0 {
            let mut trailer = String::new();
            let _ = reader.read_line(&mut trailer);
            return Ok(());
        }
        let start = out.len();
        out.resize(start + size, 0);
        reader
            .read_exact(&mut out[start..])
            .map_err(|e| format!("read chunk: {e}"))?;
        let mut crlf = [0u8; 2];
        reader
            .read_exact(&mut crlf)
            .map_err(|e| format!("read chunk crlf: {e}"))?;
    }
}
