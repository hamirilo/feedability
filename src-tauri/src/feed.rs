//! A feed's bytes into articles (RSS 0.9x / 1.0 / 2.0, Atom, JSON Feed via
//! feed-rs), and finding the feed of an ordinary web page.
//!
//! feed-rs decodes by the XML declaration's encoding, so Shift_JIS and
//! EUC-JP feeds read correctly without help.

use crate::db::NewArticle;
use feed_rs::model::{Entry, Feed};

#[derive(Debug)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub site_url: Option<String>,
    /// The feed's own icon (Atom `<icon>`) and logo (RSS `<image>`, Atom
    /// `<logo>`), absolute http(s) URLs (docs/FAVICON.md).
    pub icon: Option<String>,
    pub logo: Option<String>,
    pub articles: Vec<NewArticle>,
}

/// How much plain text an article keeps for the list and the search.
const SUMMARY_CHARS: usize = 400;

/// `url` is where the feed was fetched from: relative links resolve against it.
pub fn parse(bytes: &[u8], url: &str) -> Result<ParsedFeed, String> {
    let feed = feed_rs::parser::Builder::new()
        .base_uri(Some(url))
        .id_generator(missing_id)
        .build()
        .parse(bytes)
        .map_err(|e| format!("フィードとして読めませんでした（{e}）"))?;
    Ok(from_model(feed, url))
}

/// An id for an entry without one, the same on every refresh: from its link
/// (and title), else the feed's URL and its title, as feed-rs does. Left
/// empty when there is neither, where feed-rs would make a random one and the
/// article would come back as new each time; `article` then uses its text.
fn missing_id(
    links: &[feed_rs::model::Link],
    title: &Option<feed_rs::model::Text>,
    uri: Option<&str>,
) -> String {
    use feed_rs::parser::{generate_id_from_link_and_title, generate_id_from_uri_and_title};
    match (links.first(), uri, title) {
        (Some(link), _, _) => generate_id_from_link_and_title(link, title),
        (None, Some(uri), Some(title)) => generate_id_from_uri_and_title(uri, title),
        _ => String::new(),
    }
}

/// FNV-1a: stable across Rust versions, unlike `DefaultHasher`.
fn stable_hash(parts: &[&str]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for part in parts {
        for b in part.bytes().chain([0]) {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    format!("text:{h:016x}")
}

fn from_model(feed: Feed, base: &str) -> ParsedFeed {
    let site_url = feed
        .links
        .iter()
        .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate"))
        .map(|l| l.href.clone());
    ParsedFeed {
        title: feed
            .title
            .map(|t| plain_text(&t.content))
            .filter(|t| !t.is_empty()),
        site_url,
        icon: feed.icon.and_then(|i| absolute(base, &i.uri)),
        logo: feed.logo.and_then(|i| absolute(base, &i.uri)),
        articles: feed.entries.into_iter().map(article).collect(),
    }
}

/// `href` resolved against `base`, if it is then an http(s) URL.
fn absolute(base: &str, href: &str) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    let u = url::Url::parse(base).ok()?.join(href).ok()?;
    matches!(u.scheme(), "http" | "https").then(|| u.to_string())
}

fn article(e: Entry) -> NewArticle {
    let url = e
        .links
        .iter()
        .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate"))
        .or(e.links.first())
        .map(|l| l.href.clone());
    let summary_html = e
        .summary
        .as_ref()
        .map(|s| s.content.clone())
        .unwrap_or_default();
    let content = e
        .content
        .as_ref()
        .and_then(|c| c.body.clone())
        .unwrap_or_default();
    // Many RSS 2.0 feeds have the whole article in <description> and no
    // content: show that. Atom's summary is short when there is content.
    let body = if content.trim().is_empty() {
        summary_html.clone()
    } else {
        content
    };
    let summary = truncate(
        &plain_text(if summary_html.trim().is_empty() {
            &body
        } else {
            &summary_html
        }),
        SUMMARY_CHARS,
    );
    let title = e
        .title
        .map(|t| plain_text(&t.content))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| truncate(&summary, 60));
    let published = e.published.or(e.updated).map(|d| d.timestamp_millis());
    NewArticle {
        guid: if e.id.is_empty() {
            stable_hash(&[&body, &published.unwrap_or(0).to_string()])
        } else {
            e.id
        },
        url,
        title: if title.is_empty() {
            "（無題）".into()
        } else {
            title
        },
        author: e
            .authors
            .first()
            .and_then(|a| a.name.clone())
            .filter(|a| !a.trim().is_empty()),
        summary,
        content: body,
        published,
    }
}

fn truncate(s: &str, n: usize) -> String {
    match s.char_indices().nth(n) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

/// HTML (or text with entities) as plain text on one line: tags dropped,
/// script / style left out, entities decoded, whitespace collapsed.
pub fn plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        out.push(' ');
        let tag = &rest[lt..];
        let lower: String = tag.chars().take(8).collect::<String>().to_ascii_lowercase();
        let skip_to = if lower.starts_with("<script") {
            find_ci(tag, "</script>").map(|i| i + "</script>".len())
        } else if lower.starts_with("<style") {
            find_ci(tag, "</style>").map(|i| i + "</style>".len())
        } else if tag.starts_with("<!--") {
            tag.find("-->").map(|i| i + 3)
        } else {
            tag.find('>').map(|i| i + 1)
        };
        match skip_to {
            Some(n) => rest = &tag[n..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    let decoded = decode_entities(&out);
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    haystack.to_ascii_lowercase().find(needle)
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        let end = after.find(';').filter(|&i| i <= 10);
        let decoded = end.and_then(|i| entity(&after[..i]).map(|c| (c, i)));
        match decoded {
            Some((c, i)) => {
                out.push(c);
                rest = &after[i + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code);
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        "hellip" => '…',
        "mdash" => '—',
        "ndash" => '–',
        "laquo" => '«',
        "raquo" => '»',
        "ldquo" => '“',
        "rdquo" => '”',
        "lsquo" => '‘',
        "rsquo" => '’',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "middot" => '·',
        "bull" => '•',
        "yen" => '¥',
        _ => return None,
    })
}

/// Whether the bytes look like a web page rather than a feed.
pub fn looks_like_html(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    head.contains("<!doctype html") || head.contains("<html")
}

/// The attributes of every `<link>` in a page's start, in the page's order.
fn link_tags(html: &[u8]) -> Vec<Vec<(String, String)>> {
    // The <head> is enough, and the tags are ASCII whatever the page's encoding.
    let text = String::from_utf8_lossy(&html[..html.len().min(512 * 1024)]);
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find("<link") {
        let start = from + i;
        let Some(len) = lower[start..].find('>') else {
            break;
        };
        out.push(attributes(&text[start..start + len]));
        from = start + len;
    }
    out
}

/// The feeds a web page points to with `<link rel="alternate">`, resolved
/// against the page's URL, in the page's order.
pub fn discover(html: &[u8], page_url: &str) -> Vec<String> {
    let Ok(base) = url::Url::parse(page_url) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for attrs in link_tags(html) {
        let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        let rel_alternate = get("rel").is_some_and(|r| {
            r.split_whitespace()
                .any(|w| w.eq_ignore_ascii_case("alternate"))
        });
        let feed_type = get("type").is_some_and(|t| {
            let t = t.trim().to_ascii_lowercase();
            t == "application/rss+xml"
                || t == "application/atom+xml"
                || t == "application/feed+json"
        });
        if let (true, true, Some(href)) = (rel_alternate, feed_type, get("href")) {
            if let Ok(u) = base.join(href.trim()) {
                if matches!(u.scheme(), "http" | "https") && !found.contains(&u.to_string()) {
                    found.push(u.to_string());
                }
            }
        }
    }
    found
}

/// The icons a web page names with `<link rel="icon">` / `"shortcut icon"` /
/// `"apple-touch-icon"`, as absolute http(s) URLs, best first: an `icon` with
/// no `sizes` or a 16x16 / 32x32 one (small, what a list row needs), then any
/// other `icon`, then the apple-touch-icons. In the page's order within each.
/// SVG is left out: the app doesn't show it (fetch.rs `image_type`).
pub fn icon_links(html: &[u8], page_url: &str) -> Vec<String> {
    let Ok(base) = url::Url::parse(page_url) else {
        return Vec::new();
    };
    let mut found: Vec<(u8, String)> = Vec::new();
    for attrs in link_tags(html) {
        let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        let rel = get("rel").unwrap_or_default().to_ascii_lowercase();
        let has = |w: &str| rel.split_whitespace().any(|r| r == w);
        let is_icon = has("icon");
        let is_apple = has("apple-touch-icon") || has("apple-touch-icon-precomposed");
        if !is_icon && !is_apple {
            continue;
        }
        let Some(href) = get("href") else { continue };
        let Ok(u) = base.join(href.trim()) else {
            continue;
        };
        if !matches!(u.scheme(), "http" | "https") {
            continue;
        }
        let svg = get("type").is_some_and(|t| t.to_ascii_lowercase().contains("svg"))
            || u.path().to_ascii_lowercase().ends_with(".svg");
        if svg {
            continue;
        }
        let sizes = get("sizes").unwrap_or_default().to_ascii_lowercase();
        let small = sizes.split_whitespace().next().is_none()
            || sizes
                .split_whitespace()
                .any(|s| s == "16x16" || s == "32x32");
        let rank = match (is_icon, small) {
            (true, true) => 0,
            (true, false) => 1,
            _ => 2,
        };
        let url = u.to_string();
        if !found.iter().any(|(_, f)| *f == url) {
            found.push((rank, url));
        }
    }
    // Stable: the page's order is kept within a rank.
    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, u)| u).collect()
}

/// The attributes of a tag (`<link a="1" b='2' c=3>`), names lowercased.
fn attributes(tag: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let s = tag.trim_start_matches('<');
    let mut chars = s.char_indices().peekable();
    // Skip the tag name.
    while let Some(&(_, c)) = chars.peek() {
        if c.is_whitespace() {
            break;
        }
        chars.next();
    }
    loop {
        while chars
            .peek()
            .is_some_and(|&(_, c)| c.is_whitespace() || c == '/')
        {
            chars.next();
        }
        let Some(&(start, _)) = chars.peek() else {
            break;
        };
        while chars
            .peek()
            .is_some_and(|&(_, c)| !c.is_whitespace() && c != '=' && c != '/')
        {
            chars.next();
        }
        let end = chars.peek().map_or(s.len(), |&(i, _)| i);
        let name = s[start..end].to_ascii_lowercase();
        while chars.peek().is_some_and(|&(_, c)| c.is_whitespace()) {
            chars.next();
        }
        let mut value = String::new();
        if chars.peek().is_some_and(|&(_, c)| c == '=') {
            chars.next();
            while chars.peek().is_some_and(|&(_, c)| c.is_whitespace()) {
                chars.next();
            }
            match chars.peek().map(|&(_, c)| c) {
                Some(q @ ('"' | '\'')) => {
                    chars.next();
                    for (_, c) in chars.by_ref() {
                        if c == q {
                            break;
                        }
                        value.push(c);
                    }
                }
                _ => {
                    while let Some(&(_, c)) = chars.peek() {
                        if c.is_whitespace() {
                            break;
                        }
                        value.push(c);
                        chars.next();
                    }
                }
            }
        }
        if name.is_empty() {
            break;
        }
        out.push((name, decode_entities(&value)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel>
  <title>テストのブログ</title>
  <link>https://blog.example.com/</link>
  <item>
    <title>最初の記事 &amp; その他</title>
    <link>https://blog.example.com/1</link>
    <guid>post-1</guid>
    <pubDate>Mon, 06 Oct 2025 09:00:00 +0900</pubDate>
    <description><![CDATA[<p>本文の<b>一部</b>です。</p><script>alert(1)</script><img src="/a.png">]]></description>
  </item>
  <item>
    <description>タイトルの無い記事の本文</description>
  </item>
</channel></rss>"#;

    const ATOM: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title type="text">Atom Example</title>
  <link href="https://atom.example.com/" rel="alternate"/>
  <link href="https://atom.example.com/feed.xml" rel="self"/>
  <id>urn:example</id>
  <updated>2025-10-01T00:00:00Z</updated>
  <entry>
    <title>Entry</title>
    <id>urn:entry:1</id>
    <link href="/posts/1" rel="alternate"/>
    <updated>2025-10-01T00:00:00Z</updated>
    <author><name>Writer</name></author>
    <summary>Short</summary>
    <content type="html">&lt;p&gt;Long body&lt;/p&gt;</content>
  </entry>
</feed>"#;

    #[test]
    fn reads_rss() {
        let f = parse(RSS.as_bytes(), "https://blog.example.com/feed").unwrap();
        assert_eq!(f.title.as_deref(), Some("テストのブログ"));
        assert_eq!(f.site_url.as_deref(), Some("https://blog.example.com/"));
        let a = &f.articles[0];
        assert_eq!(a.guid, "post-1");
        assert_eq!(a.title, "最初の記事 & その他");
        assert_eq!(a.url.as_deref(), Some("https://blog.example.com/1"));
        assert_eq!(a.summary, "本文の 一部 です。");
        assert!(
            a.content.contains("<b>一部</b>"),
            "content keeps its HTML: {}",
            a.content
        );
        assert_eq!(a.published, Some(1_759_708_800_000));
        // No title: the start of the text; no guid: feed-rs makes a stable one.
        let b = &f.articles[1];
        assert_eq!(b.title, "タイトルの無い記事の本文");
        assert!(!b.guid.is_empty());
        let again = parse(RSS.as_bytes(), "https://blog.example.com/feed").unwrap();
        assert_eq!(again.articles[1].guid, b.guid);
    }

    #[test]
    fn reads_atom() {
        let f = parse(ATOM.as_bytes(), "https://atom.example.com/feed.xml").unwrap();
        assert_eq!(f.site_url.as_deref(), Some("https://atom.example.com/"));
        let a = &f.articles[0];
        assert_eq!(a.url.as_deref(), Some("https://atom.example.com/posts/1"));
        assert_eq!(a.author.as_deref(), Some("Writer"));
        assert_eq!(a.summary, "Short");
        assert_eq!(a.content, "<p>Long body</p>");
    }

    #[test]
    fn reads_the_feeds_icon_and_logo() {
        let atom = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>With icons</title>
  <id>urn:icons</id>
  <updated>2025-10-01T00:00:00Z</updated>
  <icon>https://atom.example.com/icon.png</icon>
  <logo>/logo.png</logo>
</feed>"#;
        let f = parse(atom.as_bytes(), "https://atom.example.com/feed.xml").unwrap();
        assert_eq!(f.icon.as_deref(), Some("https://atom.example.com/icon.png"));
        assert_eq!(f.logo.as_deref(), Some("https://atom.example.com/logo.png"));

        let rss = r#"<?xml version="1.0"?>
<rss version="2.0"><channel>
  <title>Blog</title>
  <link>https://blog.example.com/</link>
  <image><url>https://blog.example.com/logo.gif</url><title>Blog</title><link>https://blog.example.com/</link></image>
</channel></rss>"#;
        let f = parse(rss.as_bytes(), "https://blog.example.com/feed").unwrap();
        assert_eq!(f.logo.as_deref(), Some("https://blog.example.com/logo.gif"));
        assert_eq!(f.icon, None);

        // None declared: both empty.
        let f = parse(RSS.as_bytes(), "https://blog.example.com/feed").unwrap();
        assert_eq!((f.icon, f.logo), (None, None));
    }

    #[test]
    fn resolves_only_http_urls() {
        assert_eq!(
            absolute("https://a.example/blog/feed", "../i.png").as_deref(),
            Some("https://a.example/i.png")
        );
        assert_eq!(absolute("https://a.example/", "data:image/png;base64,AA=="), None);
        assert_eq!(absolute("https://a.example/", "  "), None);
    }

    #[test]
    fn reads_shift_jis() {
        let body = shift_jis("<?xml version=\"1.0\" encoding=\"Shift_JIS\"?><rss version=\"2.0\"><channel><title>日本語</title><item><title>記事</title><guid>1</guid></item></channel></rss>");
        let f = parse(&body, "https://example.jp/rss").unwrap();
        assert_eq!(f.title.as_deref(), Some("日本語"));
        assert_eq!(f.articles[0].title, "記事");
    }

    /// The text in Shift_JIS, as an old Japanese site would serve it.
    fn shift_jis(s: &str) -> Vec<u8> {
        encoding_rs::SHIFT_JIS.encode(s).0.into_owned()
    }

    #[test]
    fn rejects_non_feeds() {
        assert!(parse(b"<html><body>hi</body></html>", "https://example.com/").is_err());
        assert!(parse(b"", "https://example.com/").is_err());
    }

    #[test]
    fn plain_text_drops_markup() {
        assert_eq!(
            plain_text("<p>a&nbsp;b</p>\n<p>c &lt;d&gt; &#x41;&#66;</p>"),
            "a b c <d> AB"
        );
        assert_eq!(
            plain_text("x<style>.a{}</style>y<!-- c -->z<SCRIPT>s</SCRIPT>"),
            "x y z"
        );
        assert_eq!(plain_text("AT&T &unknown; & done"), "AT&T &unknown; & done");
        assert_eq!(plain_text("unclosed <b"), "unclosed");
    }

    #[test]
    fn truncates_by_characters() {
        assert_eq!(truncate("あいうえお", 3), "あいう…");
        assert_eq!(truncate("abc", 3), "abc");
    }

    #[test]
    fn finds_feeds_in_a_page() {
        let html = br#"<!DOCTYPE html><html><head>
            <link rel="stylesheet" href="/s.css">
            <LINK REL="alternate" TYPE="application/rss+xml" title="RSS" HREF="/rss.xml">
            <link rel='alternate' type='application/atom+xml' href='https://other.example/atom?a=1&amp;b=2' />
            <link rel=alternate type=application/rss+xml href=/rss.xml>
            <link rel="alternate" hreflang="en" href="/en/">
            <link rel="alternate" type="application/rss+xml" href="javascript:alert(1)">
        </head></html>"#;
        assert!(looks_like_html(html));
        assert_eq!(
            discover(html, "https://site.example/blog/"),
            [
                "https://site.example/rss.xml",
                "https://other.example/atom?a=1&b=2"
            ]
        );
        assert!(discover(b"<html></html>", "https://site.example/").is_empty());
    }

    #[test]
    fn lists_a_pages_icons_best_first() {
        let html = br#"<!DOCTYPE html><html><head>
            <link rel="apple-touch-icon" href="/apple.png">
            <link rel="icon" sizes="192x192" href="/big.png">
            <link rel="Shortcut Icon" href="/favicon.ico">
            <link rel="icon" type="image/svg+xml" href="/icon.svg">
            <link rel="icon" href="/vector.svg?v=2">
            <link rel="icon" sizes="16x16 32x32" href="https://cdn.example/small.png">
            <link rel="stylesheet" href="/s.css">
            <link rel="mask-icon" href="/mask.png">
            <link rel="icon" href="javascript:alert(1)">
            <link rel="icon" href="/favicon.ico">
            <link rel="icon">
        </head></html>"#;
        assert_eq!(
            icon_links(html, "https://site.example/blog/"),
            [
                "https://site.example/favicon.ico",
                "https://cdn.example/small.png",
                "https://site.example/big.png",
                "https://site.example/apple.png",
            ]
        );
        assert!(icon_links(b"<html></html>", "https://site.example/").is_empty());
        assert!(icon_links(html, "not a url").is_empty());
    }
}
