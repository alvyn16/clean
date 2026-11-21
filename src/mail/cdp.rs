//! Minimal Chrome DevTools Protocol client over a raw WebSocket.
//! Keeps one headless Chromium alive so rendering an email costs ~tens of ms
//! instead of paying ~400 ms process startup each time.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub struct CdpClient {
    stream: TcpStream,
    ws_key: String,
    ws_host: String,
    ws_path: String,
    next_id: AtomicU64,
}

impl CdpClient {
    /// Connect to an existing page's DevTools WebSocket endpoint.
    pub fn connect(ws_url: &str) -> Result<Self> {
        let url = url::Url::parse(ws_url)?;
        let host = url.host_str().context("no host in ws url")?;
        let port = url.port().unwrap_or(80);
        let path = format!("{}{}", url.path(), {
            let q = url.query().unwrap_or("");
            if q.is_empty() { String::new() } else { format!("?{q}") }
        });
        let mut stream = TcpStream::connect((host, port))?;
        stream.set_read_timeout(Some(Duration::from_secs(20)))?;
        let key = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            format!("{:08x}{:08x}", std::process::id(), NEXT_ID.fetch_add(1, Ordering::Relaxed)),
        );
        let request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\n\
             Upgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(request.as_bytes())?;
        let mut header = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            stream.read_exact(&mut byte)?;
            header.push(byte[0]);
            if header.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        let header_str = String::from_utf8_lossy(&header);
        if !header_str.contains("101") {
            return Err(anyhow!("websocket upgrade failed: {}", header_str.lines().next().unwrap_or("")));
        }
        Ok(Self {
            stream,
            ws_key: key,
            ws_host: format!("{host}:{port}"),
            ws_path: path,
            next_id: AtomicU64::new(1),
        })
    }

    /// Send a CDP command and return its `result` object.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let message = serde_json::json!({ "id": id, "method": method, "params": params });
        self.send_text(&message.to_string())?;

        // Loop until we see the response with our id (skip events).
        loop {
            let text = self.recv_text()?;
            let value: Value = serde_json::from_str(&text)?;
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = value.get("error") {
                    return Err(anyhow!("CDP error: {error}"));
                }
                return Ok(value.get("result").cloned().unwrap_or(Value::Null));
            }
        }
    }

    fn send_text(&mut self, text: &str) -> Result<()> {
        let data = text.as_bytes();
        let mut frame = Vec::with_capacity(data.len() + 14);
        frame.push(0x81); // FIN + text
        let len = data.len();
        if len < 126 {
            frame.push(0x80 | len as u8);
        } else if len < 65536 {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
        // Masking key (client frames must be masked).
        let mask = [0x12, 0x34, 0x56, 0x78];
        frame.extend_from_slice(&mask);
        for (i, b) in data.iter().enumerate() {
            frame.push(b ^ mask[i % 4]);
        }
        self.stream.write_all(&frame)?;
        self.stream.flush()?;
        Ok(())
    }

    pub fn recv_text(&mut self) -> Result<String> {
        let mut header = [0u8; 2];
        self.stream.read_exact(&mut header)?;
        let opcode = header[0] & 0x0f;
        let mut len = (header[1] & 0x7f) as usize;
        if len == 126 {
            let mut b = [0u8; 2];
            self.stream.read_exact(&mut b)?;
            len = u16::from_be_bytes(b) as usize;
        } else if len == 127 {
            let mut b = [0u8; 8];
            self.stream.read_exact(&mut b)?;
            len = u64::from_be_bytes(b) as usize;
        }
        if header[1] & 0x80 != 0 {
            let mut mask = [0u8; 4];
            self.stream.read_exact(&mut mask)?;
            let mut data = vec![0u8; len];
            self.stream.read_exact(&mut data)?;
            for (i, b) in data.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
            return String::from_utf8(data).context("invalid utf8 from CDP");
        }
        let mut data = vec![0u8; len];
        self.stream.read_exact(&mut data)?;
        if opcode == 0x8 {
            return Err(anyhow!("CDP websocket closed"));
        }
        String::from_utf8(data).context("invalid utf8 from CDP")
    }

    pub fn identity(&self) -> (&str, &str, &str) {
        (&self.ws_host, &self.ws_path, &self.ws_key)
    }
}
