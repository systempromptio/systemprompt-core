use std::io::{BufRead, BufReader, Read};

use super::{HttpLocalError, Result};

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
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    #[must_use]
    pub fn target(&self) -> String {
        if self.query.is_empty() {
            self.path.clone()
        } else {
            format!("{}?{}", self.path, self.query)
        }
    }
}

pub fn parse_from_read<R: Read>(reader: R) -> Result<Request> {
    let mut buf = BufReader::new(reader);
    parse_buffered(&mut buf)
}

pub fn parse_buffered<R: BufRead>(reader: &mut R) -> Result<Request> {
    let request_line = read_request_line(reader)?;
    let (method, path, query) = parse_request_line(&request_line)?;
    let parsed_headers = read_headers(reader, request_line.len())?;
    let body = read_body(
        reader,
        parsed_headers.content_length,
        parsed_headers.transfer_encoding.as_deref(),
    )?;

    Ok(Request {
        method,
        path,
        query,
        headers: parsed_headers.headers,
        body,
    })
}

struct ParsedHeaders {
    headers: Vec<(String, String)>,
    content_length: usize,
    transfer_encoding: Option<String>,
}

fn read_request_line<R: BufRead>(reader: &mut R) -> Result<String> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    Ok(line)
}

fn parse_request_line(line: &str) -> Result<(String, String, String)> {
    let mut parts = line.split_whitespace();
    let method = parts
        .next()
        .ok_or(HttpLocalError::MissingMethod)?
        .to_string();
    let raw_target = parts.next().ok_or(HttpLocalError::MissingTarget)?;
    let (path, query) = match raw_target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (raw_target.to_string(), String::new()),
    };
    Ok((method, path, query))
}

fn read_headers<R: BufRead>(reader: &mut R, initial_bytes: usize) -> Result<ParsedHeaders> {
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut total = initial_bytes;
    let mut content_length = 0usize;
    let mut transfer_encoding: Option<String> = None;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;
        total += header.len();
        if total > MAX_HEADER_BYTES {
            return Err(HttpLocalError::HeadersTooLarge);
        }
        let trimmed = header.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        let Some((name, value)) = trimmed.split_once(':') else {
            continue;
        };
        let name = name.trim().to_string();
        let value = value.trim().to_string();
        if name.eq_ignore_ascii_case("content-length") {
            content_length = value
                .parse::<usize>()
                .map_err(HttpLocalError::ParseContentLength)?;
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            transfer_encoding = Some(value.to_ascii_lowercase());
        }
        headers.push((name, value));
    }
    Ok(ParsedHeaders {
        headers,
        content_length,
        transfer_encoding,
    })
}

fn read_body<R: BufRead>(
    reader: &mut R,
    content_length: usize,
    transfer_encoding: Option<&str>,
) -> Result<Vec<u8>> {
    if content_length > MAX_BODY_BYTES {
        return Err(HttpLocalError::BodyTooLarge(content_length));
    }
    let mut body = Vec::new();
    if transfer_encoding.is_some_and(|te| te.contains("chunked")) {
        read_chunked(reader, &mut body)?;
    } else if content_length > 0 {
        body.resize(content_length, 0);
        reader.read_exact(&mut body)?;
    }
    Ok(body)
}

fn read_chunked<R: BufRead>(reader: &mut R, out: &mut Vec<u8>) -> Result<()> {
    loop {
        let mut size_line = String::new();
        reader.read_line(&mut size_line)?;
        let size_str = size_line.trim_end_matches(['\r', '\n']);
        let size_str = size_str.split(';').next().unwrap_or("0").trim();
        let size = usize::from_str_radix(size_str, 16).map_err(HttpLocalError::ParseChunkSize)?;
        if out.len() + size > MAX_BODY_BYTES {
            return Err(HttpLocalError::ChunkedBodyTooLarge);
        }
        if size == 0 {
            let mut trailer = String::new();
            let _ = reader.read_line(&mut trailer);
            return Ok(());
        }
        let start = out.len();
        out.resize(start + size, 0);
        reader.read_exact(&mut out[start..])?;
        let mut crlf = [0u8; 2];
        reader.read_exact(&mut crlf)?;
    }
}
