//! Dev IPC client: loopback-TCP messages to `morph_devrt`.
//!
//! Mirrors `morph/dev/server.py` (`IPCClient`): every payload is a JSON
//! document terminated by a NUL byte. The dev runtime announces the port it
//! bound (preferred `DEV_PORT_DEFAULT`, OS-assigned ephemeral on collision)
//! on stdout as `[morph] dev socket on 127.0.0.1:PORT`.

use std::io::Write;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{Context, Result};

/// Preferred loopback port. The dev runtime falls back to an ephemeral
/// port when this one is taken and announces the bound port on stdout.
pub const DEV_PORT_DEFAULT: u16 = 39573;

/// Prefix of the dev runtime's port announcement line.
pub const SOCKET_ANNOUNCE_PREFIX: &str = "[morph] dev socket on 127.0.0.1:";

/// Extract the announced port from a dev runtime output line.
pub fn parse_socket_announce(line: &str) -> Option<u16> {
    line.trim().strip_prefix(SOCKET_ANNOUNCE_PREFIX).and_then(|rest| rest.trim().parse().ok())
}

/// Null-terminated JSON IPC connection to the dev runtime.
pub struct IpcClient {
    stream: TcpStream,
}

impl IpcClient {
    /// Connect to the dev runtime, retrying until `timeout` elapses.
    pub fn connect(port: u16, timeout: Duration) -> Result<Self> {
        Self::connect_host("127.0.0.1", port, timeout)
    }

    /// Connect to an explicit host (loopback in production; TEST-NET in tests).
    pub fn connect_host(host: &str, port: u16, timeout: Duration) -> Result<Self> {
        let addr = format!("{host}:{port}");
        let start = std::time::Instant::now();
        loop {
            let addrs = addr
                .to_socket_addrs()
                .with_context(|| format!("resolving dev IPC address {addr}"))?;
            for sock_addr in addrs {
                if let Ok(stream) =
                    TcpStream::connect_timeout(&sock_addr, Duration::from_millis(200))
                {
                    stream.set_nodelay(true).ok();
                    return Ok(Self { stream });
                }
            }
            if start.elapsed() >= timeout {
                anyhow::bail!("timed out connecting to dev runtime at {addr}");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn send_payload(&mut self, payload: &str) -> Result<()> {
        self.stream.write_all(payload.as_bytes()).context("writing dev IPC payload")?;
        self.stream.write_all(&[0]).context("writing dev IPC NUL")?;
        self.stream.flush().context("flushing dev IPC stream")?;
        Ok(())
    }

    /// Push a full IR document (re)builds the runtime tree.
    pub fn send_ir(&mut self, ir_json: &str) -> Result<()> {
        self.send_payload(ir_json)
    }

    /// Surface a compile error in the in-app DevTools log panel.
    pub fn send_error(&mut self, message: &str) -> Result<()> {
        let envelope = serde_json::json!({ "__error__": message });
        let text = serde_json::to_string(&envelope)
            .unwrap_or_else(|_| "{\"__error__\":\"?\"}".to_string());
        self.send_payload(&text)
    }

    /// Surface a log line in the in-app DevTools log panel.
    pub fn send_log(&mut self, level: &str, message: &str) -> Result<()> {
        let envelope = serde_json::json!({ "__log__": { "level": level, "msg": message } });
        let text =
            serde_json::to_string(&envelope).unwrap_or_else(|_| "{\"__log__\":{}}".to_string());
        self.send_payload(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announce_parses_port() {
        assert_eq!(parse_socket_announce("[morph] dev socket on 127.0.0.1:39573"), Some(39573));
        assert_eq!(parse_socket_announce("[morph] dev socket on 127.0.0.1:41234\n"), Some(41234));
    }

    #[test]
    fn announce_rejects_other_lines() {
        assert_eq!(parse_socket_announce("[morph] renderer: flash"), None);
        assert_eq!(parse_socket_announce(""), None);
        assert_eq!(parse_socket_announce("[morph] dev socket on 127.0.0.1:abc"), None);
    }

    #[test]
    fn connect_times_out_without_server() {
        // TEST-NET-3 is unroutable: SYNs time out (or fail fast) and no
        // local listener can ever complete the handshake, so this never
        // races sibling tests for ephemeral loopback ports.
        let result = IpcClient::connect_host("203.0.113.1", 39573, Duration::from_millis(300));
        assert!(result.is_err());
    }

    #[test]
    fn send_ir_frames_nul_terminated_json() {
        use std::io::Read;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                sock.read_exact(&mut byte).unwrap();
                if byte[0] == 0 {
                    break;
                }
                buf.push(byte[0]);
            }
            String::from_utf8(buf).unwrap()
        });
        let mut client = IpcClient::connect(port, Duration::from_secs(5)).unwrap();
        client.send_ir(r#"{"type":"app"}"#).unwrap();
        drop(client);
        assert_eq!(accepted.join().unwrap(), r#"{"type":"app"}"#);
    }

    #[test]
    fn send_error_envelope_shape() {
        use std::io::Read;

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            loop {
                sock.read_exact(&mut byte).unwrap();
                if byte[0] == 0 {
                    break;
                }
                buf.push(byte[0]);
            }
            String::from_utf8(buf).unwrap()
        });
        let mut client = IpcClient::connect(port, Duration::from_secs(5)).unwrap();
        client.send_error("boom").unwrap();
        drop(client);
        let text = accepted.join().unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["__error__"], serde_json::Value::String("boom".to_string()));
    }
}
