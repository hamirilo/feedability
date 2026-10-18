//! The browser extension's receiver (docs/EXTENSION.md,
//! decisions/adr-0005-browser-extension-receiver.md): a small HTTP server on
//! 127.0.0.1:41630 that the extension in `extension/` talks to. It runs only
//! while the app is open and the user has turned it on.
//!
//! - Every request must carry `Authorization: Bearer <token>`. The copy of the
//!   extension the app writes out gets it in its `config.json`; any other
//!   copy asks for it with `POST /pair`, which the user allows in the app.
//! - Requests from web pages are refused: an `Origin` other than an
//!   extension's, or a `Host` other than 127.0.0.1 / localhost (DNS rebinding).
//! - The app fetches no page for the extension: a page to read later comes
//!   with its HTML, as the browser has it.
//!
//! ```text
//! POST /pair?code=1234        (no token) asks the user to connect this
//!                             extension; answers { token } once allowed
//! GET  /info                  { app, version }
//! POST /subscribe?url=        a feed's or a page's URL; { title }
//! POST /later?url=&title=     body = the page's HTML; into あとで読む
//! ```
//!
//! `/subscribe` and `/later` also take the popup's switches: `show=1` brings
//! the window to the front at what came in, `fullText=1` (subscribe only)
//! turns 全文取得 on for the new feed.

use crate::loopback::{self, Listener};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const PORT: u16 = 41630;
/// A page's HTML; the same limit as a page fetched for 全文取得.
const MAX_BODY: u64 = crate::fetch::MAX_ARTICLE_PAGE_BYTES as u64;
/// How long a refused extension waits before it may ask again (asking brings
/// the app to the front, so it mustn't be repeated at will).
const REFUSED_WAIT: Duration = Duration::from_secs(30);

/// What the server needs from the app (a trait so tests run without Tauri).
pub trait Host: Send + Sync + 'static {
    /// Subscribes to the feed at `url`, or the one its page names: the feed's title.
    fn subscribe(&self, url: &str, options: Options) -> Result<String, String>;
    /// Puts the page into あとで読む, its article found in `html`.
    fn save_page(&self, url: &str, title: &str, html: &str, options: Options)
        -> Result<(), String>;
    /// Asks the user whether to connect the extension `extension` (its id)
    /// showing `code`. Blocks until they answer (false if they don't in time).
    fn approve_pairing(&self, code: &str, extension: &str) -> bool;
}

/// The popup's switches sent with a request.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    /// Bring the window to the front at the feed or the page.
    pub show: bool,
    /// Turn 全文取得 on for the feed subscribed.
    pub full_text: bool,
}

struct Shared {
    token: String,
    /// A pairing question is open in the app (one at a time).
    pairing: AtomicBool,
    /// Extension id -> when it may ask again.
    refused: Mutex<HashMap<String, Instant>>,
}

pub struct Server {
    _listener: Listener,
}

impl Server {
    /// Starts listening on `127.0.0.1:port` (0 = any free port, for tests).
    pub fn start(port: u16, token: String, host: Arc<dyn Host>) -> Result<(Self, u16), String> {
        let shared = Arc::new(Shared {
            token,
            pairing: AtomicBool::new(false),
            refused: Mutex::new(HashMap::new()),
        });
        let listener = Listener::start(port, move |req, port| serve(req, port, &shared, &*host))?;
        let port = listener.port();
        Ok((
            Self {
                _listener: listener,
            },
            port,
        ))
    }
}

fn serve(mut req: tiny_http::Request, port: u16, shared: &Shared, host: &dyn Host) {
    let head = Head {
        method: req.method().as_str().to_ascii_uppercase(),
        url: req.url().to_string(),
        host: loopback::header(&req, "Host"),
        origin: loopback::header(&req, "Origin"),
        auth: loopback::header(&req, "Authorization"),
        length: req.body_length().map(|n| n as u64),
    };
    let (status, body) = handle(&head, req.as_reader(), port, shared, host);
    let response = tiny_http::Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes("Content-Type", "application/json; charset=utf-8")
                .unwrap(),
        )
        .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap());
    let _ = req.respond(response);
}

struct Head {
    method: String,
    url: String,
    host: Option<String>,
    origin: Option<String>,
    auth: Option<String>,
    length: Option<u64>,
}

fn error(status: u16, message: &str) -> (u16, Value) {
    (status, json!({ "error": message }))
}

const NOT_CONNECTED: &str =
    "この拡張機能は Feedability と接続されていません。拡張機能のアイコン →「アプリと接続」を押してください";
const NOT_EXTENSION: &str = "ブラウザ拡張以外からの接続は受け付けません";

fn handle(
    head: &Head,
    body: &mut dyn Read,
    port: u16,
    shared: &Shared,
    host: &dyn Host,
) -> (u16, Value) {
    if !loopback::is_own_host(head.host.as_deref(), port) {
        return error(403, "許可されていない接続です");
    }
    if head
        .origin
        .as_ref()
        .is_some_and(|o| !o.starts_with("chrome-extension://"))
    {
        return error(403, NOT_EXTENSION);
    }
    let (path, query) = head.url.split_once('?').unwrap_or((&head.url, ""));
    let params: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let param = |k: &str| {
        params
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.trim())
            .filter(|v| !v.is_empty())
    };

    if path == "/pair" {
        if head.method != "POST" {
            return error(405, "このメソッドは使えません");
        }
        // Only an extension may ask: a page is refused above, and another
        // program sends no extension Origin.
        let Some(extension) = head
            .origin
            .as_deref()
            .and_then(|o| o.strip_prefix("chrome-extension://"))
        else {
            return error(403, NOT_EXTENSION);
        };
        let code = param("code").unwrap_or("");
        if code.len() != 4 || !code.bytes().all(|b| b.is_ascii_digit()) {
            return error(400, "確認用の番号がありません");
        }
        if shared
            .refused
            .lock()
            .unwrap()
            .get(extension)
            .is_some_and(|until| Instant::now() < *until)
        {
            return error(
                429,
                "Feedability で接続が許可されませんでした。しばらく待ってから、もう一度試してください",
            );
        }
        if shared.pairing.swap(true, Ordering::SeqCst) {
            return error(
                409,
                "Feedability で別の接続を確認中です。そちらに答えてから、もう一度試してください",
            );
        }
        let approved = host.approve_pairing(code, extension);
        shared.pairing.store(false, Ordering::SeqCst);
        if !approved {
            let mut refused = shared.refused.lock().unwrap();
            refused.retain(|_, until| Instant::now() < *until);
            refused.insert(extension.to_string(), Instant::now() + REFUSED_WAIT);
            return error(403, "Feedability で接続が許可されませんでした");
        }
        return (200, json!({ "token": shared.token }));
    }

    if !loopback::has_token(head.auth.as_deref(), &shared.token) {
        return error(401, NOT_CONNECTED);
    }
    let flag = |k: &str| param(k).is_some_and(|v| v == "1");
    let options = Options {
        show: flag("show"),
        full_text: flag("fullText"),
    };
    let web_url = |k: &str| {
        param(k)
            .filter(|u| url::Url::parse(u).is_ok_and(|u| matches!(u.scheme(), "http" | "https")))
    };

    match (head.method.as_str(), path) {
        ("GET", "/info") => (
            200,
            json!({ "app": "feedability", "version": env!("CARGO_PKG_VERSION") }),
        ),
        ("POST", "/subscribe") => {
            let Some(url) = web_url("url") else {
                return error(400, "http か https の URL がありません");
            };
            match host.subscribe(url, options) {
                Ok(title) => (200, json!({ "title": title })),
                Err(e) => error(422, &e),
            }
        }
        ("POST", "/later") => {
            let Some(url) = web_url("url") else {
                return error(400, "http か https の URL がありません");
            };
            if head.length.is_some_and(|n| n > MAX_BODY) {
                return error(413, "ページが大きすぎます");
            }
            let mut data = Vec::new();
            if let Err(e) = body.take(MAX_BODY + 1).read_to_end(&mut data) {
                return error(400, &e.to_string());
            }
            if data.len() as u64 > MAX_BODY {
                return error(413, "ページが大きすぎます");
            }
            let html = String::from_utf8_lossy(&data);
            match host.save_page(url, param("title").unwrap_or(""), &html, options) {
                Ok(()) => (200, json!({})),
                Err(e) => error(500, &e),
            }
        }
        (_, "/info" | "/subscribe" | "/later") => error(405, "このメソッドは使えません"),
        _ => error(404, "見つかりません"),
    }
}

/// A new token: 32 random bytes as hex.
pub fn new_token() -> String {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("the OS gives random bytes");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const TOKEN: &str = "t0ken";

    #[derive(Default)]
    struct TestHost {
        subscribed: Mutex<Vec<(String, Options)>>,
        saved: Mutex<Vec<(String, String, String, Options)>>,
        /// Codes the user was asked about; "0000" is refused.
        asked: Mutex<Vec<String>>,
    }

    impl Host for TestHost {
        fn subscribe(&self, url: &str, options: Options) -> Result<String, String> {
            if url.contains("nofeed") {
                return Err("フィードが見つかりませんでした".into());
            }
            self.subscribed.lock().unwrap().push((url.into(), options));
            Ok("Example".into())
        }
        fn save_page(
            &self,
            url: &str,
            title: &str,
            html: &str,
            options: Options,
        ) -> Result<(), String> {
            self.saved
                .lock()
                .unwrap()
                .push((url.into(), title.into(), html.into(), options));
            Ok(())
        }
        fn approve_pairing(&self, code: &str, _extension: &str) -> bool {
            self.asked.lock().unwrap().push(code.into());
            code != "0000"
        }
    }

    fn setup() -> (Arc<TestHost>, Server, u16) {
        let host = Arc::new(TestHost::default());
        let (server, port) = Server::start(0, TOKEN.into(), host.clone()).unwrap();
        (host, server, port)
    }

    /// A raw HTTP/1.1 request; the status and the JSON answer.
    fn call(
        port: u16,
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> (u16, Value) {
        let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut req = format!(
            "{method} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: {}\r\n",
            body.len()
        );
        for (k, v) in headers {
            req += &format!("{k}: {v}\r\n");
        }
        req += "\r\n";
        s.write_all(req.as_bytes()).unwrap();
        s.write_all(body).unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).unwrap();
        let status = out[9..12].parse().unwrap();
        let json = out.split("\r\n\r\n").nth(1).unwrap_or("");
        (status, serde_json::from_str(json).unwrap_or(Value::Null))
    }

    #[test]
    fn refuses_web_pages_other_hosts_and_wrong_tokens() {
        let (_host, _server, p) = setup();
        let h = format!("127.0.0.1:{p}");
        let auth = format!("Bearer {TOKEN}");
        let ok = [("Host", h.as_str()), ("Authorization", auth.as_str())];
        assert_eq!(call(p, "GET", "/info", &ok, b"").0, 200);
        assert_eq!(call(p, "GET", "/info", &[ok[0]], b"").0, 401);
        assert_eq!(
            call(
                p,
                "GET",
                "/info",
                &[ok[0], ("Authorization", "Bearer nope")],
                b""
            )
            .0,
            401
        );
        assert_eq!(
            call(p, "GET", "/info", &[("Host", "evil.example"), ok[1]], b"").0,
            403
        );
        assert_eq!(
            call(
                p,
                "GET",
                "/info",
                &[ok[0], ok[1], ("Origin", "https://evil.example")],
                b""
            )
            .0,
            403
        );
        let ext = [
            ok[0],
            ok[1],
            ("Origin", "chrome-extension://abcdefghijklmnop"),
        ];
        let (status, info) = call(p, "GET", "/info", &ext, b"");
        assert_eq!((status, info["app"].as_str()), (200, Some("feedability")));
        assert_eq!(call(p, "GET", "/nothing", &ext, b"").0, 404);
        assert_eq!(call(p, "GET", "/subscribe", &ext, b"").0, 405);
    }

    #[test]
    fn subscribes_and_saves_pages() {
        let (host, _server, p) = setup();
        let h = format!("127.0.0.1:{p}");
        let auth = format!("Bearer {TOKEN}");
        let ext = [
            ("Host", h.as_str()),
            ("Authorization", auth.as_str()),
            ("Origin", "chrome-extension://abc"),
        ];

        let (status, res) = call(
            p,
            "POST",
            "/subscribe?url=https%3A%2F%2Fexample.com%2Ffeed",
            &ext,
            b"",
        );
        assert_eq!((status, res["title"].as_str()), (200, Some("Example")));
        let both = Options {
            show: true,
            full_text: true,
        };
        let path = "/subscribe?url=https%3A%2F%2Fexample.com%2Fnews&show=1&fullText=1";
        assert_eq!(call(p, "POST", path, &ext, b"").0, 200);
        assert_eq!(
            *host.subscribed.lock().unwrap(),
            [
                ("https://example.com/feed".to_string(), Options::default()),
                ("https://example.com/news".to_string(), both),
            ]
        );
        assert_eq!(
            call(
                p,
                "POST",
                "/subscribe?url=https%3A%2F%2Fnofeed.example",
                &ext,
                b""
            )
            .0,
            422
        );
        assert_eq!(
            call(p, "POST", "/subscribe?url=javascript%3Aalert(1)", &ext, b"").0,
            400
        );
        assert_eq!(call(p, "POST", "/subscribe", &ext, b"").0, 400);

        let html = "<html><body><p>本文</p></body></html>";
        let path = "/later?url=https%3A%2F%2Fnews.example.com%2Fa&title=%E8%A8%98%E4%BA%8B&show=1";
        assert_eq!(call(p, "POST", path, &ext, html.as_bytes()).0, 200);
        assert_eq!(
            *host.saved.lock().unwrap(),
            [(
                "https://news.example.com/a".to_string(),
                "記事".to_string(),
                html.to_string(),
                Options {
                    show: true,
                    full_text: false
                }
            )]
        );
        assert_eq!(
            call(
                p,
                "POST",
                "/later?url=file%3A%2F%2F%2Fetc%2Fpasswd",
                &ext,
                b"x"
            )
            .0,
            400
        );
    }

    #[test]
    fn pairing_hands_out_the_token_once_allowed() {
        let (host, _server, p) = setup();
        let h = format!("127.0.0.1:{p}");
        let ext = [
            ("Host", h.as_str()),
            ("Origin", "chrome-extension://abcdefghijklmnop"),
        ];

        let (status, res) = call(p, "POST", "/pair?code=4821", &ext, b"");
        assert_eq!((status, res["token"].as_str()), (200, Some(TOKEN)));
        // Refused: asking again right away doesn't bother the user.
        assert_eq!(call(p, "POST", "/pair?code=0000", &ext, b"").0, 403);
        assert_eq!(call(p, "POST", "/pair?code=4821", &ext, b"").0, 429);
        // Only extensions, with 4 digits, by POST.
        assert_eq!(
            call(p, "POST", "/pair?code=1234", &[ext[0]], b"").0,
            403,
            "no Origin"
        );
        assert_eq!(
            call(
                p,
                "POST",
                "/pair?code=12",
                &[ext[0], ("Origin", "chrome-extension://x")],
                b""
            )
            .0,
            400
        );
        assert_eq!(call(p, "GET", "/pair?code=1234", &ext, b"").0, 405);
        assert_eq!(*host.asked.lock().unwrap(), ["4821", "0000"]);
    }

    #[test]
    fn tokens_are_long_and_new() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}
