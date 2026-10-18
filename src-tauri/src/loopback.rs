//! A small HTTP server on 127.0.0.1, for the browser extension's receiver
//! (extension.rs; the MCP server of phase 8 will use it too). Copied from
//! stockroom (src-tauri/src/loopback.rs), as the local-server standard says.
//! Each request is handled on its own thread; dropping the `Listener` closes
//! the port.

use std::sync::Arc;
use std::thread::JoinHandle;

pub struct Listener {
    http: Option<Arc<tiny_http::Server>>,
    thread: Option<JoinHandle<()>>,
    port: u16,
}

/// tiny_http closes the listening socket on its own thread after the server is
/// dropped, without waiting for it: the port can stay open for a moment.
const CLOSE_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

impl Listener {
    /// Starts listening on `127.0.0.1:port` (0 = any free port, for tests).
    /// `handle` gets every request and the port actually listened on.
    pub fn start(
        port: u16,
        handle: impl Fn(tiny_http::Request, u16) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let ip = std::net::Ipv4Addr::LOCALHOST;
        // Restarting (a new key) can come right after the old server let go of the port.
        let started = std::time::Instant::now();
        let http = loop {
            match tiny_http::Server::http((ip, port)) {
                Ok(h) => break h,
                Err(e)
                    if started.elapsed() < CLOSE_WAIT
                        && e.downcast_ref::<std::io::Error>().map(|e| e.kind()) == Some(std::io::ErrorKind::AddrInUse) =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(format!(
                        "{ip}:{port} で待ち受けできませんでした（ほかのアプリが使用中の可能性があります）: {e}"
                    ))
                }
            }
        };
        let http = Arc::new(http);
        let port = http.server_addr().to_ip().map(|a| a.port()).unwrap_or(port);
        let handle = Arc::new(handle);
        let server = http.clone();
        let thread = std::thread::spawn(move || {
            for req in server.incoming_requests() {
                let handle = handle.clone();
                // Subscribing and pairing can take a while; don't hold up other requests.
                std::thread::spawn(move || handle(req, port));
            }
        });
        Ok(Self {
            http: Some(http),
            thread: Some(thread),
            port,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for Listener {
    /// Returns once the port is closed, so it can be listened on again at once.
    fn drop(&mut self) {
        if let Some(http) = self.http.take() {
            http.unblock();
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
            drop(http); // the last reference: tiny_http starts closing the socket
        }
        let started = std::time::Instant::now();
        while started.elapsed() < CLOSE_WAIT
            && std::net::TcpStream::connect(("127.0.0.1", self.port)).is_ok()
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

/// A header of the request, if present.
pub fn header(req: &tiny_http::Request, name: &'static str) -> Option<String> {
    req.headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str().to_string())
}

/// The `Host` header names this server (not a rebinding DNS name).
pub fn is_own_host(host: Option<&str>, port: u16) -> bool {
    let allowed = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    host.is_some_and(|h| allowed.iter().any(|a| a.eq_ignore_ascii_case(h)))
}

/// `Authorization: Bearer <token>` carries `token`. Compares without
/// stopping at the first difference.
pub fn has_token(auth: Option<&str>, token: &str) -> bool {
    let given = auth
        .and_then(|a| a.strip_prefix("Bearer "))
        .unwrap_or("")
        .trim();
    !token.is_empty()
        && given.len() == token.len()
        && given
            .bytes()
            .zip(token.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}
