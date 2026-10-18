//! Sending an article to stockroom (docs/STOCKROOM.md,
//! decisions/adr-0004-sending-to-stockroom.md): stockroom's receiver on
//! 127.0.0.1:41620, the one its browser extension uses, and nothing else.
//!
//! - Only `BASE`. Never through a proxy (an office PAC can send 127.0.0.1
//!   elsewhere), never from the WebView. This is not `fetch.rs`: that one is
//!   the way out to the internet.
//! - The token comes from stockroom's own connection question (`pair`): a
//!   4-digit code shown in both apps, allowed in stockroom.
//! - Only when the user sends (or opens the settings to choose where to):
//!   nothing is asked at launch or with a refresh.

use serde::Deserialize;
use std::time::Duration;

pub const BASE: &str = "http://127.0.0.1:41620";
/// How stockroom's question names this app (`X-Stockroom-Client`).
const CLIENT_NAME: &str = "Feedability";

/// stockroom's settings page for its receiver is ブラウザ拡張 (it was 連携
/// before stockroom 0.28): the extension's receiver is the one apps use too.
pub const NOT_RUNNING: &str =
    "stockroom を起動し、設定（⌘,）›「ブラウザ拡張」で「ブラウザ拡張からの保存を受け付ける」をオンにしてください";
const NO_LIBRARY: &str = "stockroom でライブラリを開いてください";
const NO_FOLDER: &str = "送り先のフォルダが見つかりません。設定 › stockroom で選び直してください";

/// What went wrong, as the user reads it; `Unauthorized`: the token is no
/// longer good (made again in stockroom), connect again.
#[derive(Debug, PartialEq)]
pub enum Error {
    Unauthorized,
    Other(String),
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::Other(s)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

fn client(timeout: Duration) -> reqwest::Client {
    // The same TLS provider as fetch.rs, though this is plain http.
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(3))
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("a plain client builds")
}

/// stockroom's answer read: its `error` in its own words, the statuses
/// docs/STOCKROOM.md lists in ours.
async fn read(res: reqwest::Result<reqwest::Response>) -> Result<serde_json::Value> {
    let res = res.map_err(|e| {
        if e.is_connect() {
            Error::Other(NOT_RUNNING.into())
        } else if e.is_timeout() {
            Error::Other("stockroom から応答がありませんでした".into())
        } else {
            Error::Other(format!("stockroom に送れませんでした（{e}）"))
        }
    })?;
    let status = res.status().as_u16();
    let bytes = res.bytes().await.unwrap_or_default();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    if (200..300).contains(&status) {
        return Ok(body);
    }
    let said = body["error"].as_str().unwrap_or_default().to_string();
    Err(match status {
        401 => Error::Unauthorized,
        503 => Error::Other(NO_LIBRARY.into()),
        400 if said.contains("フォルダ") => Error::Other(NO_FOLDER.into()),
        _ if !said.is_empty() => Error::Other(format!("stockroom：{said}")),
        _ => Error::Other(format!("stockroom がエラーを返しました（HTTP {status}）")),
    })
}

fn url(base: &str, path: &str, query: &[(&str, &str)]) -> String {
    let mut u = url::Url::parse(base)
        .expect("BASE parses")
        .join(path)
        .expect("a path");
    if !query.is_empty() {
        u.query_pairs_mut().extend_pairs(query);
    }
    u.to_string()
}

/// Asks stockroom to connect, showing `code`; waits while the user answers
/// there (stockroom gives up after 2 minutes). The token on success.
pub async fn pair(base: &str, code: &str) -> Result<String> {
    let res = client(Duration::from_secs(130))
        .post(url(base, "/pair", &[("code", code)]))
        .header("X-Stockroom-Client", CLIENT_NAME)
        .send()
        .await;
    let body = read(res).await.map_err(|e| match e {
        // 403 here is stockroom saying no, not a lost token.
        Error::Other(s) if s.contains("許可されません") => {
            Error::Other("stockroom で接続が許可されませんでした".into())
        }
        e => e,
    })?;
    body["token"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::Other("stockroom から接続の鍵が届きませんでした".into()))
}

async fn get(base: &str, token: &str, path: &str) -> Result<serde_json::Value> {
    read(
        client(Duration::from_secs(10))
            .get(url(base, path, &[]))
            .bearer_auth(token)
            .send()
            .await,
    )
    .await
}

/// That it is stockroom answering on the port (ADR-0004), before sending.
pub async fn check(base: &str, token: &str) -> Result<()> {
    let info = get(base, token, "/info").await?;
    if info["app"] != "stockroom" {
        return Err(Error::Other(
            "127.0.0.1:41620 で応答したのは stockroom ではありません".into(),
        ));
    }
    if info["library"].is_null() {
        return Err(Error::Other(NO_LIBRARY.into()));
    }
    Ok(())
}

#[derive(Deserialize, serde::Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
}

/// The stock's folders, in stockroom's sidebar order.
pub async fn folders(base: &str, token: &str) -> Result<Vec<Folder>> {
    serde_json::from_value(get(base, token, "/folders").await?)
        .map_err(|e| Error::Other(format!("stockroom のフォルダを読めませんでした（{e}）")))
}

pub async fn tags(base: &str, token: &str) -> Result<Vec<String>> {
    serde_json::from_value(get(base, token, "/tags").await?)
        .map_err(|e| Error::Other(format!("stockroom のタグを読めませんでした（{e}）")))
}

pub struct Item<'a> {
    pub name: &'a str,
    pub markdown: &'a str,
    /// The article's URL: stockroom's 取り込み元.
    pub page: Option<&'a str>,
    pub folder: Option<&'a str>,
    pub tags: &'a [String],
}

/// Saves the item; true when stockroom already had the same text.
pub async fn import(base: &str, token: &str, item: &Item<'_>) -> Result<bool> {
    let mut query = vec![("name", item.name)];
    query.extend(item.page.map(|p| ("page", p)));
    query.extend(item.folder.map(|f| ("folder", f)));
    query.extend(item.tags.iter().map(|t| ("tag", t.as_str())));
    let body = read(
        client(Duration::from_secs(30))
            .post(url(base, "/import", &query))
            .bearer_auth(token)
            .header(
                reqwest::header::CONTENT_TYPE,
                "text/markdown; charset=utf-8",
            )
            .body(item.markdown.to_string())
            .send()
            .await,
    )
    .await?;
    Ok(body["imported"].as_u64() == Some(0) && body["duplicates"].as_u64().unwrap_or(0) > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// A stand-in for stockroom on 127.0.0.1 answering one request with
    /// `status` and `json`; hands back the request it got.
    fn serve_once(status: &str, json: &str) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
            json.len()
        );
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(300)))
                .unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 8192];
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
                if got.windows(4).any(|w| w == b"\r\n\r\n") && !got.starts_with(b"POST /import") {
                    break;
                }
            }
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8_lossy(&got).into_owned()
        });
        (base, handle)
    }

    fn block<T>(f: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(f)
    }

    #[test]
    fn pairs_by_name_and_code() {
        let (base, server) = serve_once("200 OK", r#"{"token":"t0k"}"#);
        assert_eq!(block(pair(&base, "4821")), Ok("t0k".into()));
        let req = server.join().unwrap().to_lowercase();
        assert!(req.starts_with("post /pair?code=4821 "), "{req}");
        assert!(req.contains("x-stockroom-client: feedability"), "{req}");
        assert!(!req.contains("origin:"), "{req}");

        let (base, server) = serve_once(
            "403 Forbidden",
            r#"{"error":"アプリで接続が許可されませんでした"}"#,
        );
        assert_eq!(
            block(pair(&base, "4821")),
            Err(Error::Other(
                "stockroom で接続が許可されませんでした".into()
            ))
        );
        server.join().unwrap();
    }

    #[test]
    fn sends_the_markdown_with_where_it_goes() {
        let (base, server) = serve_once("200 OK", r#"{"imported":1,"duplicates":0,"id":"x"}"#);
        let tags = vec!["あとで".to_string(), "rss".to_string()];
        let item = Item {
            name: "記事 & タイトル.md",
            markdown: "# 記事\n\n本文\n",
            page: Some("https://example.com/post?id=1"),
            folder: Some("f1"),
            tags: &tags,
        };
        assert_eq!(block(import(&base, "t0k", &item)), Ok(false));
        let req = server.join().unwrap();
        let line = req.lines().next().unwrap();
        // Encoded so stockroom's parse_query reads them back as they were.
        assert!(line.contains("name=%E8%A8%98%E4%BA%8B+%26+"), "{line}");
        assert!(
            line.contains("page=https%3A%2F%2Fexample.com%2Fpost%3Fid%3D1"),
            "{line}"
        );
        assert!(
            line.contains("folder=f1&tag=%E3%81%82%E3%81%A8%E3%81%A7&tag=rss"),
            "{line}"
        );
        assert!(
            req.to_lowercase().contains("authorization: bearer t0k"),
            "{req}"
        );
        assert!(req.ends_with("# 記事\n\n本文\n"), "{req}");

        let (base, server) = serve_once("200 OK", r#"{"imported":0,"duplicates":1,"id":"x"}"#);
        assert_eq!(block(import(&base, "t0k", &item)), Ok(true));
        server.join().unwrap();
    }

    #[test]
    fn says_what_stockroom_needs() {
        let cases = [
            ("401 Unauthorized", r#"{"error":"x"}"#, Error::Unauthorized),
            (
                "503 Service Unavailable",
                r#"{"error":"x"}"#,
                Error::Other(NO_LIBRARY.into()),
            ),
            (
                "400 Bad Request",
                r#"{"error":"保存先のフォルダが見つかりません。拡張機能で選び直してください"}"#,
                Error::Other(NO_FOLDER.into()),
            ),
            (
                "422 Unprocessable Entity",
                r#"{"error":"読み込めませんでした"}"#,
                Error::Other("stockroom：読み込めませんでした".into()),
            ),
        ];
        for (status, json, want) in cases {
            let (base, server) = serve_once(status, json);
            assert_eq!(block(get(&base, "t", "/folders")).err(), Some(want));
            server.join().unwrap();
        }
        // Nothing listening: stockroom isn't running or its receiver is off.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert_eq!(
            block(get(&format!("http://127.0.0.1:{port}"), "t", "/info")).err(),
            Some(Error::Other(NOT_RUNNING.into()))
        );
    }

    #[test]
    fn checks_it_is_stockroom_with_a_library() {
        let (base, server) = serve_once(
            "200 OK",
            r#"{"app":"stockroom","version":"1","library":"Lib"}"#,
        );
        assert_eq!(block(check(&base, "t")), Ok(()));
        server.join().unwrap();
        let (base, server) = serve_once("200 OK", r#"{"app":"other"}"#);
        assert!(block(check(&base, "t")).is_err());
        server.join().unwrap();
        let (base, server) = serve_once("200 OK", r#"{"app":"stockroom","library":null}"#);
        assert_eq!(
            block(check(&base, "t")),
            Err(Error::Other(NO_LIBRARY.into()))
        );
        server.join().unwrap();
    }
}
