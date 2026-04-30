use std::io::{Read, Write};
use std::net::TcpStream;

use super::Result;

fn reason_phrase(status: u16) -> &'static str {
    match status {
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        _ => "OK",
    }
}

pub struct ResponseBuilder<'a> {
    status: u16,
    content_type: &'a str,
    body: &'a [u8],
    nosniff: bool,
}

impl<'a> ResponseBuilder<'a> {
    #[must_use]
    pub fn new(status: u16) -> Self {
        Self {
            status,
            content_type: "text/plain",
            body: &[],
            nosniff: false,
        }
    }

    #[must_use]
    pub fn content_type(mut self, ct: &'a str) -> Self {
        self.content_type = ct;
        self
    }

    #[must_use]
    pub fn body(mut self, bytes: &'a [u8]) -> Self {
        self.body = bytes;
        self
    }

    #[must_use]
    pub fn nosniff(mut self) -> Self {
        self.nosniff = true;
        self
    }

    pub fn write(self, stream: &mut TcpStream) -> std::io::Result<()> {
        let reason = reason_phrase(self.status);
        let nosniff_header = if self.nosniff {
            "X-Content-Type-Options: nosniff\r\n"
        } else {
            ""
        };
        let header = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: \
             {len}\r\nCache-Control: no-store\r\n{nosniff}Connection: close\r\n\r\n",
            status = self.status,
            ct = self.content_type,
            len = self.body.len(),
            nosniff = nosniff_header,
        );
        stream.write_all(header.as_bytes())?;
        if !self.body.is_empty() {
            stream.write_all(self.body)?;
        }
        stream.flush()
    }
}

pub fn write_chunked(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    headers: &[(String, String)],
    body: &mut dyn Read,
) -> Result<()> {
    let mut had_content_length = false;
    let mut had_transfer_encoding = false;
    let mut head = format!("HTTP/1.1 {status} {reason}\r\n");
    for (name, value) in headers {
        if name.eq_ignore_ascii_case("content-length") {
            had_content_length = true;
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            had_transfer_encoding = true;
        }
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    let stream_chunked = !had_content_length && !had_transfer_encoding;
    if stream_chunked {
        head.push_str("Transfer-Encoding: chunked\r\n");
    }
    head.push_str("Connection: close\r\n\r\n");
    stream.write_all(head.as_bytes())?;
    stream.flush().ok();

    let mut buf = [0u8; 4096];
    if stream_chunked {
        loop {
            match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk_header = format!("{n:x}\r\n");
                    stream.write_all(chunk_header.as_bytes())?;
                    stream.write_all(&buf[..n])?;
                    stream.write_all(b"\r\n")?;
                    stream.flush().ok();
                },
                Err(e) => return Err(e.into()),
            }
        }
        stream.write_all(b"0\r\n\r\n")?;
        stream.flush().ok();
    } else {
        loop {
            match body.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    stream.write_all(&buf[..n])?;
                    stream.flush().ok();
                },
                Err(e) => return Err(e.into()),
            }
        }
    }
    Ok(())
}
