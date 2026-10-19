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
    // A new parser each time: one that failed keeps half its Media RSS state.
    let parser = || {
        feed_rs::parser::Builder::new()
            .base_uri(Some(url))
            .id_generator(missing_id)
            .build()
    };
    let feed = match parser().parse(bytes) {
        Ok(feed) => feed,
        // feed-rs 3 rejects the whole feed over one empty Media RSS
        // `<media:description>` or `<media:title>` (TechnoEdge has one on
        // every item). They say nothing, so drop them and try once more.
        Err(e) => match without_empty_media_text(bytes) {
            Some(fixed) => parser().parse(fixed.as_slice()),
            None => Err(e),
        }
        .map_err(|e| format!("フィードとして読めませんでした（{e}）"))?,
    };
    Ok(from_model(feed, url))
}

/// The feed without its empty `<media:title>` and `<media:description>`
/// (`<x/>`, `<x></x>`, `<x><![CDATA[]]></x>`), or None when it has none.
/// Works on the bytes: the tags are ASCII, and no byte of a Shift_JIS or
/// EUC-JP character is `<`, `>` or `/`.
fn without_empty_media_text(bytes: &[u8]) -> Option<Vec<u8>> {
    fn skip_ws(b: &[u8], mut i: usize) -> usize {
        while b.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        i
    }
    // Where the empty element starting at `i` (at its `<`) ends, if it is one.
    fn empty_end(b: &[u8], i: usize, name: &[u8]) -> Option<usize> {
        let mut j = i + 1 + name.len();
        if !b
            .get(j)
            .is_some_and(|c| c.is_ascii_whitespace() || b"/>".contains(c))
        {
            return None;
        }
        j += b[j..].iter().position(|&c| c == b'>')?;
        if b[j - 1] == b'/' {
            return Some(j + 1);
        }
        j = skip_ws(b, j + 1);
        if b[j..].starts_with(b"<![CDATA[") {
            j = skip_ws(b, j + 9);
            j = skip_ws(b, j + b[j..].strip_prefix(b"]]>".as_slice()).map(|_| 3)?);
        }
        j = skip_ws(b, j + b[j..].strip_prefix(b"</".as_slice()).map(|_| 2)?);
        j = skip_ws(b, j + b[j..].strip_prefix(name).map(|_| name.len())?);
        (b.get(j) == Some(&b'>')).then_some(j + 1)
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let end = [b"media:description".as_slice(), b"media:title"]
            .iter()
            .filter(|name| bytes[i] == b'<' && bytes[i + 1..].starts_with(name))
            .find_map(|name| empty_end(bytes, i, name));
        match end {
            Some(end) => i = end,
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    (out.len() < bytes.len()).then_some(out)
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
        articles: feed.entries.into_iter().map(|e| article(e, base)).collect(),
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

/// `base` is the feed's URL, for an article without its own.
fn article(e: Entry, base: &str) -> NewArticle {
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
    let thumb_url = thumbnail(&e.media, &body, url.as_deref().unwrap_or(base));
    // YouTube has nothing in either: its text is in <media:group>.
    let body = if body.trim().is_empty() {
        media_body(&e.media, thumb_url.as_deref(), url.as_deref())
    } else {
        body
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
        thumb_url,
    }
}

/// The picture for the list's 画像付き layout (docs/LIST_LAYOUT.md): the
/// feed's own thumbnail (`<media:thumbnail>` next to a `<media:content>`;
/// feed-rs drops a lone one), else an
/// image it attaches (`<media:content>`, `<enclosure>`), else the first
/// `<img>` of the body. Relative URLs resolve against the article's URL, as
/// in the reader (sanitize.rs). SVG is left out: the app doesn't show it.
fn thumbnail(media: &[feed_rs::model::MediaObject], body: &str, base: &str) -> Option<String> {
    let usable = |u: &str| absolute(base, u).filter(|u| !is_svg(u));
    let declared = media
        .iter()
        .flat_map(|m| &m.thumbnails)
        .find_map(|t| usable(&t.image.uri))
        .or_else(|| {
            media
                .iter()
                .flat_map(|m| &m.content)
                .filter(|c| {
                    c.content_type
                        .as_ref()
                        .is_some_and(|t| t.to_string().starts_with("image/"))
                })
                .find_map(|c| c.url.as_ref().and_then(|u| usable(u.as_str())))
        });
    declared.or_else(|| {
        tags(body.as_bytes(), "img").into_iter().find_map(|attrs| {
            let get = |k: &str| attrs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
            // A 1x1 counter or a spacer, not a picture of the article.
            let tiny = |k: &str| {
                get(k)
                    .and_then(|v| v.trim().parse::<u32>().ok())
                    .is_some_and(|n| n <= 2)
            };
            if tiny("width") || tiny("height") {
                return None;
            }
            usable(get("src")?)
        })
    })
}

/// The body of an item whose only text is Media RSS (a YouTube video): its
/// picture, linked to the article (the video plays in the browser), then its
/// `<media:description>`. Empty when it has neither.
fn media_body(
    media: &[feed_rs::model::MediaObject],
    thumb: Option<&str>,
    link: Option<&str>,
) -> String {
    let mut out = String::new();
    if let Some(src) = thumb {
        let img = format!(r#"<img src="{}" alt="">"#, escape(src));
        out += &match link {
            Some(href) => format!(r#"<p><a href="{}">{img}</a></p>"#, escape(href)),
            None => format!("<p>{img}</p>"),
        };
    }
    let description = media
        .iter()
        .filter_map(|m| m.description.as_ref())
        .find(|d| !d.content.trim().is_empty());
    if let Some(d) = description {
        if d.content_type.to_string().starts_with("text/html") {
            out += &d.content;
        } else {
            out += &text_html(&d.content);
        }
    }
    out
}

/// Plain text as HTML: a blank line between paragraphs, a line break
/// within one, and its http(s) URLs as links.
fn text_html(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let mut out = String::new();
    for para in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        let lines: Vec<String> = para.lines().map(linkify).collect();
        out += &format!("<p>{}</p>", lines.join("<br>"));
    }
    out
}

/// A line of plain text as HTML, its http(s) URLs as links. A URL ends at
/// whitespace or a character outside ASCII (Japanese text often follows one
/// without a space), less any punctuation that closes the sentence.
fn linkify(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(i) = ["https://", "http://"]
        .iter()
        .filter_map(|p| rest.find(p))
        .min()
    {
        out += &escape(&rest[..i]);
        let tail = &rest[i..];
        let len = tail
            .find(|c: char| c.is_whitespace() || !c.is_ascii() || "<>\"".contains(c))
            .unwrap_or(tail.len());
        let url = tail[..len].trim_end_matches(['.', ',', ';', ':', '!', '?', ')', '\'']);
        out += &format!(r#"<a href="{0}">{0}</a>"#, escape(url));
        rest = &tail[url.len()..];
    }
    out + &escape(rest)
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn is_svg(u: &str) -> bool {
    url::Url::parse(u).is_ok_and(|u| u.path().to_ascii_lowercase().ends_with(".svg"))
}

/// A page saved from the browser extension (docs/EXTENSION.md) as an
/// article: `body` is what `fulltext::extract` found in it (empty when
/// nothing was), the page's URL is its id.
pub fn page_article(url: &str, title: &str, body: &str, now: i64) -> NewArticle {
    let summary = truncate(&plain_text(body), SUMMARY_CHARS);
    let title = plain_text(title);
    NewArticle {
        guid: url.to_string(),
        url: Some(url.to_string()),
        title: if title.is_empty() {
            url.to_string()
        } else {
            title
        },
        author: None,
        summary,
        content: body.to_string(),
        published: Some(now),
        thumb_url: thumbnail(&[], body, url),
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
    tags(html, "link")
}

/// The attributes of every `<name ...>` tag in the first 512 KB, in order.
fn tags(html: &[u8], name: &str) -> Vec<Vec<(String, String)>> {
    let text = String::from_utf8_lossy(&html[..html.len().min(512 * 1024)]);
    let lower = text.to_ascii_lowercase();
    let open = format!("<{name}");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find(&open) {
        let start = from + i;
        let Some(len) = lower[start..].find('>') else {
            break;
        };
        from = start + len;
        // `<img` is not `<image`.
        let after = lower[start + open.len()..].chars().next();
        if after.is_some_and(|c| c.is_whitespace() || c == '/' || c == '>') {
            out.push(attributes(&text[start..start + len]));
        }
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

fn is_youtube(u: &url::Url) -> bool {
    u.host_str().is_some_and(|h| {
        let h = h.to_ascii_lowercase();
        h == "youtube.com" || h.ends_with(".youtube.com")
    })
}

/// The feed of a YouTube channel or playlist that its URL alone names
/// (`/channel/UC…`, `/user/…`, `…?list=…`), so the page isn't fetched: in
/// the EU it can be a consent page with nothing in it (docs/YOUTUBE.md).
pub fn youtube_feed(page_url: &str) -> Option<String> {
    let u = url::Url::parse(page_url).ok()?;
    if !is_youtube(&u) || u.path().starts_with("/feeds/") {
        return None;
    }
    let feed = |key: &str, value: &str| {
        let mut f = url::Url::parse("https://www.youtube.com/feeds/videos.xml").ok()?;
        f.query_pairs_mut().append_pair(key, value);
        Some(f.to_string())
    };
    let mut segments = u.path_segments()?.filter(|s| !s.is_empty());
    match (segments.next(), segments.next()) {
        (Some("channel"), Some(id)) if is_channel_id(id) => feed("channel_id", id),
        (Some("user"), Some(name)) => feed("user", name),
        (Some("playlist"), _) => {
            let list = u.query_pairs().find(|(k, _)| k == "list")?.1;
            feed("playlist_id", &list)
        }
        _ => None,
    }
}

/// The feed of the YouTube channel a page is of (`/@name`, `/c/…`) or
/// belongs to (a video), from the channel id in it: for a page that names
/// no feed with `<link rel="alternate">`.
pub fn youtube_feed_in_page(html: &[u8], page_url: &str) -> Option<String> {
    if !url::Url::parse(page_url).is_ok_and(|u| is_youtube(&u)) {
        return None;
    }
    let text = String::from_utf8_lossy(html);
    // The page's own channel comes before the ones it recommends.
    ["\"externalId\":\"", "\"channelId\":\""]
        .iter()
        .find_map(|key| {
            let start = text.find(key)? + key.len();
            let id = text.get(start..start + 24)?;
            is_channel_id(id).then(|| id.to_string())
        })
        .and_then(|id| youtube_feed(&format!("https://www.youtube.com/channel/{id}")))
}

/// `UC` and 22 characters of base64url.
fn is_channel_id(s: &str) -> bool {
    s.len() == 24
        && s.starts_with("UC")
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
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
    fn reads_a_feed_with_empty_media_text() {
        // As TechnoEdge's (www.techno-edge.net/rss20/index.rdf) writes it.
        let xml = r#"<?xml version="1.0" encoding="UTF-8" ?>
<rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/"><channel><title>T</title>
  <item><title>A</title><link>https://blog.example.com/a</link>
    <media:content url="https://blog.example.com/a.jpg" type="image/jpeg" medium="image">
      <media:description type="html"><![CDATA[]]></media:description>
      <media:title></media:title><media:title/>
      <media:copyright>c</media:copyright>
    </media:content>
  </item></channel></rss>"#;
        let f = parse(xml.as_bytes(), "https://blog.example.com/feed").unwrap();
        assert_eq!(f.articles[0].title, "A");
        assert_eq!(
            f.articles[0].thumb_url.as_deref(),
            Some("https://blog.example.com/a.jpg")
        );
        // A description with text is left as it is.
        let kept = "<media:description>x</media:description><media:titles/>";
        assert_eq!(without_empty_media_text(kept.as_bytes()), None);
    }

    #[test]
    fn finds_the_articles_picture() {
        let thumb = |item: &str| {
            let xml = format!(
                r#"<?xml version="1.0"?><rss version="2.0" xmlns:media="http://search.yahoo.com/mrss/"><channel><title>T</title>
                <item><title>A</title><link>https://blog.example.com/2025/a</link>{item}</item></channel></rss>"#
            );
            parse(xml.as_bytes(), "https://blog.example.com/feed")
                .unwrap()
                .articles[0]
                .thumb_url
                .clone()
        };
        // The feed's own thumbnail comes first. (feed-rs keeps one only
        // next to a <media:content>: a lone <media:thumbnail> is dropped.)
        assert_eq!(
            thumb(r#"<media:group><media:content url="https://cdn.example.com/v.mp4" type="video/mp4"/><media:thumbnail url="https://cdn.example.com/t.jpg"/></media:group><description>&lt;img src="b.png"&gt;</description>"#)
                .as_deref(),
            Some("https://cdn.example.com/t.jpg")
        );
        // Then an attached image, not an attached sound.
        assert_eq!(
            thumb(r#"<enclosure url="https://cdn.example.com/a.mp3" type="audio/mpeg" length="1"/><enclosure url="https://cdn.example.com/e.png" type="image/png" length="1"/>"#)
                .as_deref(),
            Some("https://cdn.example.com/e.png")
        );
        // Then the first real <img> of the body, relative to the article.
        assert_eq!(
            thumb(r#"<description>&lt;img src="/px.gif" width="1" height="1"&gt;&lt;image src="x"&gt;&lt;IMG alt="x" src="img/b.png"&gt;</description>"#)
                .as_deref(),
            Some("https://blog.example.com/2025/img/b.png")
        );
        assert_eq!(
            thumb(r#"<description>&lt;img src="logo.svg"&gt;</description>"#),
            None
        );
        assert_eq!(thumb("<description>text only</description>"), None);
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
        assert_eq!(
            absolute("https://a.example/", "data:image/png;base64,AA=="),
            None
        );
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

    const YOUTUBE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
 <link rel="self" href="http://www.youtube.com/feeds/videos.xml?channel_id=UCabcdefghijklmnopqrstuv"/>
 <id>yt:channel:abcdefghijklmnopqrstuv</id>
 <title>Example Channel</title>
 <link rel="alternate" href="https://www.youtube.com/channel/UCabcdefghijklmnopqrstuv"/>
 <entry>
  <id>yt:video:VID123</id>
  <yt:videoId>VID123</yt:videoId>
  <title>動画のタイトル</title>
  <link rel="alternate" href="https://www.youtube.com/watch?v=VID123"/>
  <author><name>Example Channel</name></author>
  <published>2026-10-01T00:00:00+00:00</published>
  <media:group>
   <media:title>動画のタイトル</media:title>
   <media:content url="https://www.youtube.com/v/VID123?version=3" type="application/x-shockwave-flash" width="640" height="390"/>
   <media:thumbnail url="https://i1.ytimg.com/vi/VID123/hqdefault.jpg" width="480" height="360"/>
   <media:description>説明の 1 行目 &lt;b&gt;
詳しくはhttps://example.com/a?x=1&amp;y=2。(https://example.com/b)

2 段落目</media:description>
  </media:group>
 </entry>
</feed>"#;

    #[test]
    fn youtube_description_is_the_body() {
        let url = "https://www.youtube.com/feeds/videos.xml?channel_id=UCabcdefghijklmnopqrstuv";
        let feed = parse(YOUTUBE.as_bytes(), url).unwrap();
        let a = &feed.articles[0];
        assert_eq!(a.title, "動画のタイトル");
        assert_eq!(
            a.thumb_url.as_deref(),
            Some("https://i1.ytimg.com/vi/VID123/hqdefault.jpg")
        );
        assert_eq!(
            a.content,
            concat!(
                r#"<p><a href="https://www.youtube.com/watch?v=VID123"><img src="https://i1.ytimg.com/vi/VID123/hqdefault.jpg" alt=""></a></p>"#,
                r#"<p>説明の 1 行目 &lt;b&gt;<br>詳しくは<a href="https://example.com/a?x=1&amp;y=2">https://example.com/a?x=1&amp;y=2</a>。(<a href="https://example.com/b">https://example.com/b</a>)</p>"#,
                "<p>2 段落目</p>"
            )
        );
        assert!(
            a.summary.starts_with("説明の 1 行目 <b> 詳しくは"),
            "{}",
            a.summary
        );
    }

    #[test]
    fn media_body_only_when_the_feed_has_no_text() {
        let xml = YOUTUBE.replace(
            "<title>動画のタイトル</title>",
            "<title>動画のタイトル</title><summary>要約</summary>",
        );
        let feed = parse(xml.as_bytes(), "https://www.youtube.com/feeds/videos.xml").unwrap();
        assert_eq!(feed.articles[0].content, "要約");
    }

    #[test]
    fn youtube_feed_from_the_url() {
        let feed = |u| youtube_feed(u);
        assert_eq!(
            feed("https://www.youtube.com/channel/UCabcdefghijklmnopqrstuv/videos").as_deref(),
            Some("https://www.youtube.com/feeds/videos.xml?channel_id=UCabcdefghijklmnopqrstuv")
        );
        assert_eq!(
            feed("https://m.youtube.com/user/someone").as_deref(),
            Some("https://www.youtube.com/feeds/videos.xml?user=someone")
        );
        assert_eq!(
            feed("https://www.youtube.com/playlist?list=PL123&si=x").as_deref(),
            Some("https://www.youtube.com/feeds/videos.xml?playlist_id=PL123")
        );
        // A handle or a video names no channel: the page is fetched.
        assert_eq!(feed("https://www.youtube.com/@someone"), None);
        assert_eq!(feed("https://www.youtube.com/watch?v=VID123"), None);
        assert_eq!(feed("https://www.youtube.com/channel/not-an-id"), None);
        assert_eq!(
            feed("https://www.youtube.com/feeds/videos.xml?user=someone"),
            None
        );
        assert_eq!(
            feed("https://notyoutube.com/channel/UCabcdefghijklmnopqrstuv"),
            None
        );
    }

    #[test]
    fn youtube_feed_from_the_page() {
        let page = br#"<html><script>var ytInitialData = {"metadata":{"channelMetadataRenderer":{"externalId":"UCabcdefghijklmnopqrstuv"}},"x":{"channelId":"UCzzzzzzzzzzzzzzzzzzzzzz"}}</script></html>"#;
        assert_eq!(
            youtube_feed_in_page(page, "https://www.youtube.com/@someone").as_deref(),
            Some("https://www.youtube.com/feeds/videos.xml?channel_id=UCabcdefghijklmnopqrstuv")
        );
        let video = br#"<script>{"videoDetails":{"videoId":"VID123","channelId":"UCzzzzzzzzzzzzzzzzzzzzzz"}}</script>"#;
        assert_eq!(
            youtube_feed_in_page(video, "https://www.youtube.com/watch?v=VID123").as_deref(),
            Some("https://www.youtube.com/feeds/videos.xml?channel_id=UCzzzzzzzzzzzzzzzzzzzzzz")
        );
        assert_eq!(youtube_feed_in_page(page, "https://site.example/"), None);
        assert_eq!(
            youtube_feed_in_page(b"<html>consent</html>", "https://consent.youtube.com/m"),
            None
        );
    }
}
