//! 全文取得 (docs/FULL_TEXT.md): the article out of its page, for feeds that
//! only carry a summary. The page comes from `fetch::get_page`; the article
//! is found by dom_smoothie (a port of Firefox's Readability.js, see
//! docs/research/readability.md) and is shown through `sanitize::article_html`
//! like any feed's HTML. This is the one place that uses dom_smoothie.

use dom_smoothie::{Config, Readability};

/// More elements than this and the page isn't read: a page that big is not
/// an article, and parsing it would hold up the other fetches. News pages
/// measured 2,000–6,000.
const MAX_ELEMENTS: usize = 30_000;

/// Fewer characters than this and the extraction counts as failed (a login
/// wall, a page drawn by JavaScript, a cookie notice): the summary is shown.
const MIN_CHARS: usize = 100;

/// The page's text in its own encoding: the charset in the Content-Type,
/// else a BOM or the page's `<meta charset>`, else UTF-8.
pub fn decode(body: &[u8], content_type: Option<&str>) -> String {
    let label = content_type
        .and_then(charset_in)
        .or_else(|| meta_charset(body));
    let encoding = label
        .and_then(|l| encoding_rs::Encoding::for_label(l.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    // A BOM wins over the label, as in a browser.
    encoding.decode(body).0.into_owned()
}

fn charset_in(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|p| {
        let (k, v) = p.split_once('=')?;
        k.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| v.trim().trim_matches(['"', '\'']).to_string())
    })
}

/// `<meta charset="…">` or `<meta http-equiv="Content-Type" content="…; charset=…">`
/// in the first 2 KB (the tags are ASCII whatever the encoding).
fn meta_charset(body: &[u8]) -> Option<String> {
    let head = String::from_utf8_lossy(&body[..body.len().min(2048)]).to_ascii_lowercase();
    let at = head.find("charset=")? + "charset=".len();
    let rest = head[at..].trim_start_matches(['"', '\'']);
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or(rest.len());
    (end > 0).then(|| rest[..end].to_string())
}

/// The article's HTML from its page, or None when there is no article to be
/// found. `url` is the page's URL after redirects (relative links resolve
/// against it later, in `sanitize::article_html`).
pub fn extract(html: &str, url: &str) -> Option<String> {
    let cfg = Config {
        max_elements_to_parse: MAX_ELEMENTS,
        ..Default::default()
    };
    let mut r = Readability::new(html, Some(url), Some(cfg)).ok()?;
    let article = r.parse().ok()?;
    let chars = article
        .text_content
        .chars()
        .filter(|c| !c.is_whitespace())
        .count();
    (chars >= MIN_CHARS).then(|| article.content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(body: &str) -> String {
        format!(
            r#"<!doctype html><html><head><meta charset="utf-8"><title>記事のタイトル | Example</title></head>
            <body>
              <nav><a href="/">ホーム</a> <a href="/news">ニュース</a> <a href="/about">このサイトについて</a></nav>
              {body}
              <footer>Copyright Example</footer>
            </body></html>"#
        )
    }

    #[test]
    fn finds_the_article_and_leaves_the_rest() {
        let para = "これは記事の本文の段落です。ページの中で、いちばん文章が多いところが記事として選ばれます。".repeat(3);
        let html = page(&format!(
            r#"<article><h1>記事のタイトル</h1><p>{para}</p><p>{para}</p><img src="/img/a.png"><p>{para}</p></article>
               <aside><h2>関連記事</h2><ul><li><a href="/1">ほかの記事</a></li></ul></aside>"#
        ));
        let out = extract(&html, "https://example.com/news/1").expect("an article");
        assert!(out.contains("これは記事の本文の段落です"), "{out}");
        assert!(out.contains("<img"), "{out}");
        assert!(!out.contains("このサイトについて"), "{out}");
        assert!(!out.contains("Copyright"), "{out}");
    }

    #[test]
    fn too_little_text_is_no_article() {
        let html = page("<main><p>ログインしてください。</p></main>");
        assert_eq!(extract(&html, "https://example.com/news/1"), None);
        // A page drawn by JavaScript.
        let html = page(r#"<div id="root"></div><script>render()</script>"#);
        assert_eq!(extract(&html, "https://example.com/news/1"), None);
    }

    #[test]
    fn reads_the_page_in_its_encoding() {
        let text = "<p>日本語のページ</p>";
        let (sjis, _, _) = encoding_rs::SHIFT_JIS.encode(text);
        assert_eq!(decode(&sjis, Some("text/html; charset=Shift_JIS")), text);
        assert_eq!(
            decode(&sjis, Some("text/html; charset=\"shift_jis\"")),
            text
        );

        let mut meta =
            br#"<html><head><meta http-equiv="Content-Type" content="text/html; charset=EUC-JP">"#
                .to_vec();
        let (euc, _, _) = encoding_rs::EUC_JP.encode(text);
        meta.extend_from_slice(&euc);
        assert!(decode(&meta, Some("text/html")).ends_with(text));

        let mut meta = br#"<meta charset='shift_jis'>"#.to_vec();
        meta.extend_from_slice(&sjis);
        assert!(decode(&meta, None).ends_with(text));

        assert_eq!(decode(text.as_bytes(), None), text);
    }
}
