//! Minimal, dependency-free HTTP/1.1 client (plain `http://`).
//!
//! ROXY talks to local gateways (OpenCode, the Antigravity/Codex proxy) over
//! loopback HTTP, so a full TLS stack would only add weight and build time.
//! Streaming + chunked decoding is implemented so SSE token streams arrive live.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub struct Target {
    pub host: String,
    pub port: u16,
    pub path: String,
}

pub fn parse_http(url: &str) -> Result<Target, String> {
    let url = url.trim();
    let rest = if let Some(r) = url.strip_prefix("http://") {
        r
    } else if url.starts_with("https://") {
        return Err("https endpoint — ROXY speaks http:// to local gateways".into());
    } else {
        url
    };

    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };

    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>().map_err(|_| format!("bad port in {url}"))?,
        ),
        None => (hostport.to_string(), 80),
    };

    if host.is_empty() {
        return Err(format!("bad url {url}"));
    }
    Ok(Target {
        host,
        port,
        path: path.to_string(),
    })
}

enum Framing {
    Chunked,
    Fixed(u64),
    Close,
}

pub struct Conn {
    reader: BufReader<TcpStream>,
    framing: Framing,
    left: u64,
    done: bool,
}

impl Conn {
    pub fn open(
        method: &str,
        url: &str,
        key: Option<&str>,
        body: Option<&str>,
        connect_timeout: Duration,
        read_timeout: Duration,
    ) -> Result<(u16, Vec<(String, String)>, Conn), String> {
        let t = parse_http(url)?;
        let addr = format!("{}:{}", t.host, t.port);
        let sock = addr
            .to_socket_addrs()
            .map_err(|e| format!("resolve {addr}: {e}"))?
            .next()
            .ok_or_else(|| format!("resolve {addr}: no address"))?;
        let stream = TcpStream::connect_timeout(&sock, connect_timeout)
            .map_err(|e| format!("connect {addr}: {e}"))?;
        stream
            .set_read_timeout(Some(read_timeout))
            .map_err(|e| e.to_string())?;
        stream.set_nodelay(true).map_err(|e| e.to_string())?;

        let mut req = String::new();
        req.push_str(&format!("{method} {} HTTP/1.1\r\n", t.path));
        req.push_str(&format!("Host: {}:{}\r\n", t.host, t.port));
        req.push_str("User-Agent: ROXY/0.1\r\n");
        req.push_str("Accept: application/json, text/event-stream\r\n");
        req.push_str("Accept-Encoding: identity\r\n");
        req.push_str("Connection: close\r\n");
        if let Some(k) = key {
            if !k.trim().is_empty() {
                req.push_str(&format!("Authorization: Bearer {}\r\n", k.trim()));
            }
        }
        if let Some(b) = body {
            req.push_str("Content-Type: application/json\r\n");
            req.push_str(&format!("Content-Length: {}\r\n", b.as_bytes().len()));
        }
        req.push_str("\r\n");
        if let Some(b) = body {
            req.push_str(b);
        }

        let mut stream = stream;
        stream
            .write_all(req.as_bytes())
            .map_err(|e| format!("write: {e}"))?;
        stream.flush().map_err(|e| e.to_string())?;

        let mut reader = BufReader::with_capacity(16 * 1024, stream);

        let mut status_line = String::new();
        reader
            .read_line(&mut status_line)
            .map_err(|e| format!("read status: {e}"))?;
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| format!("bad status line: {status_line:?}"))?;

        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
            if n == 0 || line == "\r\n" || line == "\n" {
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
            }
        }

        let get = |name: &str| -> Option<String> {
            headers
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone())
        };

        let framing = if get("transfer-encoding")
            .map(|v| v.to_ascii_lowercase().contains("chunked"))
            .unwrap_or(false)
        {
            Framing::Chunked
        } else if let Some(len) = get("content-length").and_then(|v| v.parse::<u64>().ok()) {
            Framing::Fixed(len)
        } else {
            Framing::Close
        };

        let left = match framing {
            Framing::Fixed(n) => n,
            _ => 0,
        };

        Ok((
            status,
            headers,
            Conn {
                reader,
                framing,
                left,
                done: false,
            },
        ))
    }

    /// Pull the next slice of decoded body bytes. Returns 0 at EOF.
    fn read_raw(&mut self, out: &mut [u8]) -> Result<usize, String> {
        if self.done {
            return Ok(0);
        }
        match self.framing {
            Framing::Chunked => {
                if self.left == 0 {
                    let mut line = String::new();
                    if self
                        .reader
                        .read_line(&mut line)
                        .map_err(|e| e.to_string())?
                        == 0
                    {
                        self.done = true;
                        return Ok(0);
                    }
                    let hex = line.trim().split(';').next().unwrap_or("0");
                    let size = u64::from_str_radix(hex.trim(), 16).unwrap_or(0);
                    if size == 0 {
                        // drain trailers
                        loop {
                            let mut t = String::new();
                            let n = self.reader.read_line(&mut t).unwrap_or(0);
                            if n == 0 || t == "\r\n" || t == "\n" {
                                break;
                            }
                        }
                        self.done = true;
                        return Ok(0);
                    }
                    self.left = size;
                }
                let want = std::cmp::min(out.len() as u64, self.left) as usize;
                let n = self
                    .reader
                    .read(&mut out[..want])
                    .map_err(|e| format!("read: {e}"))?;
                if n == 0 {
                    self.done = true;
                    return Ok(0);
                }
                self.left -= n as u64;
                if self.left == 0 {
                    let mut crlf = String::new();
                    let _ = self.reader.read_line(&mut crlf);
                }
                Ok(n)
            }
            Framing::Fixed(_) => {
                if self.left == 0 {
                    self.done = true;
                    return Ok(0);
                }
                let want = std::cmp::min(out.len() as u64, self.left) as usize;
                let n = self
                    .reader
                    .read(&mut out[..want])
                    .map_err(|e| format!("read: {e}"))?;
                if n == 0 {
                    self.done = true;
                    return Ok(0);
                }
                self.left -= n as u64;
                Ok(n)
            }
            Framing::Close => {
                let n = self.reader.read(out).map_err(|e| format!("read: {e}"))?;
                if n == 0 {
                    self.done = true;
                }
                Ok(n)
            }
        }
    }

    /// Read one line of the body (without trailing newline).
    pub fn read_body_line(&mut self) -> Result<Option<String>, String> {
        let mut buf = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            let n = self.read_raw(&mut byte)?;
            if n == 0 {
                if buf.is_empty() {
                    return Ok(None);
                }
                break;
            }
            if byte[0] == b'\n' {
                if buf.last() == Some(&b'\r') {
                    buf.pop();
                }
                break;
            }
            buf.push(byte[0]);
            if buf.len() > 1024 * 1024 {
                break;
            }
        }
        Ok(Some(String::from_utf8_lossy(&buf).to_string()))
    }

    pub fn read_body_all(&mut self) -> Result<String, String> {
        let mut out = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = self.read_raw(&mut chunk)?;
            if n == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..n]);
            if out.len() > 16 * 1024 * 1024 {
                break;
            }
        }
        Ok(String::from_utf8_lossy(&out).to_string())
    }
}

/// One-shot JSON request. Returns (status, body).
pub fn json(
    method: &str,
    url: &str,
    key: Option<&str>,
    body: Option<&str>,
    timeout: Duration,
) -> Result<(u16, String), String> {
    let (status, _headers, mut conn) = Conn::open(
        method,
        url,
        key,
        body,
        timeout.min(Duration::from_secs(6)),
        timeout,
    )?;
    let text = conn.read_body_all()?;
    Ok((status, text))
}
