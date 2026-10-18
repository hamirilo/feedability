//! Fetching feeds and article images over HTTP.
//!
//! This is the app's one reach outside besides the updater (decisions/
//! adr-0001-fetching-feeds.md): only URLs the user subscribed to, the feeds
//! their pages point to, the images inside their articles, and the icons of
//! the feeds' sites, and, for feeds with 全文取得 on, their articles' pages
//! (`get_page`, which refuses addresses on this machine and its network).
//!
//! - Through the proxy the OS would use for that URL, PAC included (proxy.rs).
//!   reqwest alone only knows fixed proxies and environment variables.
//! - Bodies are read up to a limit; a server that keeps sending is cut off
//!   instead of filling memory (release builds abort on allocation failure).
//! - Conditional requests (ETag / Last-Modified): an unchanged feed costs a 304.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// A feed bigger than this is not one anybody reads.
pub const MAX_FEED_BYTES: usize = 10 * 1024 * 1024;
/// Images in articles; larger ones stay as links.
pub const MAX_IMAGE_BYTES: usize = 15 * 1024 * 1024;
/// A site's icon (a favicon is a few KB; 256 KB is generous).
pub const MAX_ICON_BYTES: usize = 256 * 1024;
/// A site's front page, read only for its `<link rel="icon">` tags.
pub const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;
/// An article's page, read for 全文取得 (docs/FULL_TEXT.md).
pub const MAX_ARTICLE_PAGE_BYTES: usize = 5 * 1024 * 1024;

const USER_AGENT: &str = concat!(
    "Feedability/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/hamirilo/feedability)"
);

pub struct Response {
    pub body: Vec<u8>,
    /// Where the body came from after redirects (relative links resolve against it).
    pub url: String,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub enum Outcome {
    Fetched(Response),
    /// 304: the feed hasn't changed since `etag` / `last_modified`.
    NotModified,
}

/// `http(s)://…` checked; a bare "example.com/feed" becomes https.
pub fn normalize_url(input: &str) -> Result<String, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("URL を入力してください".into());
    }
    let with_scheme = if s.contains("://") {
        s.to_string()
    } else {
        format!("https://{s}")
    };
    let url = url::Url::parse(&with_scheme).map_err(|_| format!("URL として読めません：{s}"))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none_or(str::is_empty) {
        return Err(format!("http か https の URL を入力してください：{s}"));
    }
    Ok(url.to_string())
}

/// Whether the URL names this machine or the local network outright:
/// `localhost`, or an IP literal that is loopback, private (10/8, 172.16/12,
/// 192.168/16, fc00::/7), link-local or unspecified. A name that resolves to
/// such an address isn't caught (that would take replacing reqwest's
/// resolver; decisions/adr-0001-fetching-feeds.md).
pub fn is_local(url: &url::Url) -> bool {
    use std::net::{Ipv4Addr, Ipv6Addr};
    let v4 = |ip: Ipv4Addr| {
        ip.is_loopback()
            || ip.is_private()
            || ip.is_link_local()
            || ip.is_unspecified()
            || ip.is_broadcast()
    };
    let v6 = |ip: Ipv6Addr| {
        let first = ip.segments()[0];
        ip.is_loopback()
            || ip.is_unspecified()
            || (first & 0xfe00) == 0xfc00
            || (first & 0xffc0) == 0xfe80
            || ip.to_ipv4_mapped().is_some_and(v4)
    };
    match url.host() {
        Some(url::Host::Domain(d)) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d == "localhost" || d.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(ip)) => v4(ip),
        Some(url::Host::Ipv6(ip)) => v6(ip),
        None => true,
    }
}

/// One client per proxy (a reqwest client holds its proxy and connection
/// pool), and per whether redirects to local addresses are refused.
type ClientKey = (Option<String>, bool);
static CLIENTS: Mutex<Vec<(ClientKey, reqwest::Client)>> = Mutex::new(Vec::new());
/// The OS's proxy answer per host, for a few minutes: running the PAC script
/// for every request of a 200-feed refresh is slow, and a laptop that moves
/// between the office and home gets a fresh answer soon enough.
/// When the OS was asked about a host, and its answer.
type ProxyAnswer = (Instant, Option<String>);
static PROXIES: Mutex<Option<HashMap<String, ProxyAnswer>>> = Mutex::new(None);
const PROXY_TTL: Duration = Duration::from_secs(300);

async fn proxy_for(url: &url::Url) -> Option<String> {
    let host = url.host_str().unwrap_or_default().to_string();
    if let Some((at, p)) = PROXIES
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&host))
        .cloned()
    {
        if at.elapsed() < PROXY_TTL {
            return p;
        }
    }
    let target = url.to_string();
    let found = tauri::async_runtime::spawn_blocking(move || crate::proxy::for_url(&target))
        .await
        .ok()
        .flatten();
    PROXIES
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(host, (Instant::now(), found.clone()));
    found
}

fn client(proxy: Option<String>, remote_only: bool) -> Result<reqwest::Client, String> {
    let key = (proxy, remote_only);
    let mut clients = CLIENTS.lock().unwrap();
    if let Some((_, c)) = clients.iter().find(|(k, _)| *k == key) {
        return Ok(c.clone());
    }
    let proxy = key.0.clone();
    // reqwest is built without a default TLS crypto provider (the updater's
    // choice, which this shares): ring, installed once for the process.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let mut b = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(30))
        .redirect(if remote_only {
            reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 8 {
                    attempt.error("too many redirects")
                } else if is_local(attempt.url()) {
                    attempt.error(LocalAddress)
                } else {
                    attempt.follow()
                }
            })
        } else {
            reqwest::redirect::Policy::limited(8)
        });
    // None: reqwest's own lookup (environment variables, fixed system
    // proxies), as the updater does; only PAC needs the OS's answer.
    if let Some(p) = &proxy {
        b = b.proxy(reqwest::Proxy::all(p).map_err(|e| e.to_string())?);
    }
    let c = b.build().map_err(|e| e.to_string())?;
    clients.push((key, c.clone()));
    Ok(c)
}

/// A redirect to this machine or the local network, refused by `get_page`.
#[derive(Debug)]
struct LocalAddress;

impl std::fmt::Display for LocalAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(LOCAL_REFUSED)
    }
}

impl std::error::Error for LocalAddress {}

const LOCAL_REFUSED: &str = "この PC や手元のネットワークのアドレスは取得しません";

/// What the user reads when a request fails.
fn describe(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "応答がありませんでした（タイムアウト）".into()
    } else if e.is_connect() {
        "サーバーに接続できませんでした".into()
    } else if e.is_redirect() {
        let local = std::error::Error::source(e)
            .is_some_and(|s| s.downcast_ref::<LocalAddress>().is_some());
        if local {
            LOCAL_REFUSED.into()
        } else {
            "転送が多すぎます".into()
        }
    } else {
        format!("取得できませんでした（{e}）")
    }
}

pub async fn get(
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    accept: &str,
    max: usize,
) -> Result<Outcome, String> {
    request(url, etag, last_modified, accept, max, false).await
}

/// An article's page for 全文取得: HTML only, and never this machine or the
/// local network, redirects included. The link comes from whoever runs the
/// feed, and unlike an image, what comes back is kept and shown.
pub async fn get_page(url: &str) -> Result<Response, String> {
    let res = match request(url, None, None, PAGE_ACCEPT, MAX_ARTICLE_PAGE_BYTES, true).await? {
        Outcome::Fetched(res) => res,
        Outcome::NotModified => return Err("取得できませんでした（HTTP 304）".into()),
    };
    let html = res.content_type.as_deref().is_none_or(|t| {
        let t = t.to_ascii_lowercase();
        t.contains("text/html") || t.contains("application/xhtml")
    });
    if !html {
        return Err("HTML のページではありません".into());
    }
    Ok(res)
}

async fn request(
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    accept: &str,
    max: usize,
    remote_only: bool,
) -> Result<Outcome, String> {
    let parsed = url::Url::parse(url).map_err(|_| format!("URL として読めません：{url}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(format!("http か https の URL ではありません：{url}"));
    }
    if remote_only && is_local(&parsed) {
        return Err(LOCAL_REFUSED.into());
    }
    let client = client(proxy_for(&parsed).await, remote_only)?;
    let mut req = client.get(parsed).header(reqwest::header::ACCEPT, accept);
    if let Some(v) = etag {
        req = req.header(reqwest::header::IF_NONE_MATCH, v);
    }
    if let Some(v) = last_modified {
        req = req.header(reqwest::header::IF_MODIFIED_SINCE, v);
    }
    let mut res = req.send().await.map_err(|e| describe(&e))?;
    let status = res.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Outcome::NotModified);
    }
    if !status.is_success() {
        return Err(match status.as_u16() {
            404 | 410 => format!("見つかりません（HTTP {}）", status.as_u16()),
            401 | 403 => format!("アクセスが拒否されました（HTTP {}）", status.as_u16()),
            _ => format!("サーバーがエラーを返しました（HTTP {}）", status.as_u16()),
        });
    }
    if res.content_length().is_some_and(|n| n > max as u64) {
        return Err(too_big(max));
    }
    let header = |name: reqwest::header::HeaderName| {
        res.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let content_type = header(reqwest::header::CONTENT_TYPE);
    let etag = header(reqwest::header::ETAG);
    let last_modified = header(reqwest::header::LAST_MODIFIED);
    let final_url = res.url().to_string();
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await.map_err(|e| describe(&e))? {
        if body.len() + chunk.len() > max {
            return Err(too_big(max));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(Outcome::Fetched(Response {
        body,
        url: final_url,
        content_type,
        etag,
        last_modified,
    }))
}

fn too_big(max: usize) -> String {
    format!("大きすぎるため読みません（上限 {} MB）", max / 1024 / 1024)
}

pub const FEED_ACCEPT: &str =
    "application/rss+xml, application/atom+xml, application/feed+json, application/xml;q=0.9, text/xml;q=0.9, text/html;q=0.5, */*;q=0.1";

/// For a site's icon: no SVG (`image_type` drops it anyway).
pub const ICON_ACCEPT: &str =
    "image/png, image/x-icon, image/vnd.microsoft.icon, image/webp, image/jpeg, image/gif, image/*;q=0.8, */*;q=0.1";

/// For a site's page, where a feed would be the wrong answer.
pub const PAGE_ACCEPT: &str = "text/html, application/xhtml+xml;q=0.9, */*;q=0.1";

/// The image's type from its first bytes. The server's Content-Type isn't
/// trusted (security standard §4): only formats the WebView decodes as a
/// plain image are passed on; SVG never (it can reference other files).
pub fn image_type(bytes: &[u8]) -> Option<&'static str> {
    let starts = |p: &[u8]| bytes.starts_with(p);
    if starts(b"\xFF\xD8\xFF") {
        Some("image/jpeg")
    } else if starts(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if starts(b"GIF87a") || starts(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && matches!(&bytes[8..12], b"avif" | b"avis")
    {
        Some("image/avif")
    } else if starts(b"BM") {
        Some("image/bmp")
    } else if starts(b"\0\0\x01\0") {
        Some("image/x-icon")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_urls() {
        assert_eq!(
            normalize_url(" example.com/feed ").unwrap(),
            "https://example.com/feed"
        );
        assert_eq!(
            normalize_url("http://example.com").unwrap(),
            "http://example.com/"
        );
        assert!(normalize_url("").is_err());
        assert!(normalize_url("file:///etc/passwd").is_err());
        assert!(normalize_url("javascript:alert(1)").is_err());
        assert!(normalize_url("https://").is_err());
    }

    /// A server on 127.0.0.1 that answers one request with `response` and
    /// hands back the request it got.
    fn serve_once(response: Vec<u8>) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/feed", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = stream.read(&mut buf).unwrap();
            let _ = stream.write_all(&response);
            String::from_utf8_lossy(&buf[..n]).to_lowercase()
        });
        (url, handle)
    }

    fn get_blocking(url: &str, etag: Option<&str>, max: usize) -> Result<Outcome, String> {
        tauri::async_runtime::block_on(get(url, etag, None, FEED_ACCEPT, max))
    }

    #[test]
    fn sends_the_etag_and_takes_a_304() {
        let (url, server) =
            serve_once(b"HTTP/1.1 304 Not Modified\r\nContent-Length: 0\r\n\r\n".to_vec());
        let out = get_blocking(&url, Some("\"v1\""), MAX_FEED_BYTES).unwrap();
        assert!(matches!(out, Outcome::NotModified));
        let request = server.join().unwrap();
        assert!(request.contains("if-none-match: \"v1\""), "{request}");
        assert!(request.contains("user-agent: feedability/"), "{request}");
    }

    #[test]
    fn reads_the_body_and_its_headers() {
        let body = "<rss/>";
        let (url, server) = serve_once(
            format!("HTTP/1.1 200 OK\r\nContent-Type: application/rss+xml\r\nETag: \"v2\"\r\nContent-Length: {}\r\n\r\n{body}", body.len())
                .into_bytes(),
        );
        let Outcome::Fetched(res) = get_blocking(&url, None, MAX_FEED_BYTES).unwrap() else {
            panic!("304")
        };
        assert_eq!(res.body, body.as_bytes());
        assert_eq!(res.etag.as_deref(), Some("\"v2\""));
        assert_eq!(res.content_type.as_deref(), Some("application/rss+xml"));
        server.join().unwrap();
    }

    #[test]
    fn stops_reading_past_the_limit() {
        // No Content-Length: the limit has to hold while reading.
        let mut response = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
        response.extend(std::iter::repeat_n(b'x', 64 * 1024));
        let (url, server) = serve_once(response);
        let err = get_blocking(&url, None, 1024).err().unwrap();
        assert!(err.contains("大きすぎる"), "{err}");
        server.join().unwrap();
    }

    #[test]
    fn says_what_went_wrong() {
        let (url, server) =
            serve_once(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec());
        assert_eq!(
            get_blocking(&url, None, MAX_FEED_BYTES).err().unwrap(),
            "見つかりません（HTTP 404）"
        );
        server.join().unwrap();
        assert!(get_blocking("ftp://example.com/feed", None, MAX_FEED_BYTES).is_err());
    }

    #[test]
    fn tells_local_addresses() {
        let local = |u: &str| is_local(&url::Url::parse(u).unwrap());
        for u in [
            "http://localhost/",
            "http://LOCALHOST./x",
            "http://app.localhost:3000/",
            "http://127.0.0.1:41620/import",
            "http://10.1.2.3/",
            "http://172.16.0.1/",
            "http://192.168.1.1/admin",
            "http://169.254.169.254/latest/meta-data",
            "http://0.0.0.0/",
            "http://[::1]/",
            "http://[fd00::1]/",
            "http://[fe80::1]/",
            "http://[::ffff:192.168.0.1]/",
        ] {
            assert!(local(u), "{u}");
        }
        for u in [
            "https://example.com/",
            "https://8.8.8.8/",
            "https://172.32.0.1/",
            "https://[2001:db8::1]/",
            "https://localhost.example.com/",
        ] {
            assert!(!local(u), "{u}");
        }
    }

    #[test]
    fn a_page_on_this_machine_is_refused() {
        // Nothing is asked: the server would hang on accept if it were.
        let err = tauri::async_runtime::block_on(get_page("http://127.0.0.1:9/post")).err();
        assert_eq!(err.as_deref(), Some(LOCAL_REFUSED));
    }

    #[test]
    fn tells_images_by_content() {
        assert_eq!(image_type(b"\xFF\xD8\xFF\xE0rest"), Some("image/jpeg"));
        assert_eq!(image_type(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
        assert_eq!(image_type(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(image_type(b"\0\0\0\x1cftypavif"), Some("image/avif"));
        assert_eq!(
            image_type(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            None
        );
        assert_eq!(image_type(b"<html>"), None);
        assert_eq!(image_type(b""), None);
    }
}
