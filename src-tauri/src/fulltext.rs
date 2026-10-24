//! 全文取得 (docs/FULL_TEXT.md): the article out of its page, for feeds that
//! only carry a summary. The page comes from `fetch::get_page`; the article
//! is found by dom_smoothie (a port of Firefox's Readability.js, see
//! docs/research/readability.md) and is shown through `sanitize::article_html`
//! like any feed's HTML. This is the one place that uses dom_smoothie.

use dom_query::Document;
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
        classes_to_preserve: [
            "fa-talk",
            "fa-talk-r",
            "fa-talk-icon",
            "fa-talk-name",
            "fa-talk-text",
        ]
        .map(String::from)
        .to_vec(),
        ..Default::default()
    };
    let doc = Document::from(html);
    talk_balloons(&doc);
    let mut r = Readability::with_document(doc, Some(url), Some(cfg)).ok()?;
    let article = r.parse().ok()?;
    let chars = article
        .text_content
        .chars()
        .filter(|c| !c.is_whitespace())
        .count();
    (chars >= MIN_CHARS).then(|| article.content.to_string())
}

/// Talk balloons get our classes (sanitize::talk_class) before Readability
/// reads the page, or it drops `voicecomment` as a comment section and strips
/// the classes the reader lays them out by. It also folds a div of only an
/// image or a few words into a <p>, losing the class, so the icon and the
/// name stop being divs; and a div around a single div hands its class down
/// over the inner one's (SWELL's wrapper), so a wrapper's class goes.
fn talk_balloons(doc: &Document) {
    for el in doc.select("div[class], p[class]").iter() {
        let Some(class) = el
            .attr("class")
            .and_then(|c| crate::sanitize::talk_class(&c))
        else {
            continue;
        };
        el.set_attr("class", &class);
        let part = class.split(' ').next().unwrap_or_default();
        match part {
            "fa-talk" => {
                let parent = el.parent();
                if parent.is("div") && parent.children().length() == 1 {
                    parent.remove_attr("class");
                }
            }
            "fa-talk-icon" if el.is("div") => el.rename("figure"),
            "fa-talk-name" => el.rename("figcaption"),
            _ => {}
        }
    }
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

    /// One balloon of each theme, as their own sites write them (2026-10):
    /// (theme, name, words, the HTML).
    const BALLOONS: &[(&str, &str, &str, &str)] = &[
        (
            "OPENCAGE",
            "まの",
            "オープンケージの言葉です。",
            r#"<div class="voice cf r"><figure class="icon"><img src="/a.png"><figcaption class="name">まの</figcaption></figure><div class="voicecomment"><p>オープンケージの言葉です。</p></div></div>"#,
        ),
        (
            "Cocoon",
            "わいひら",
            "コクーンの言葉です。",
            r#"<div class="speech-wrap sb-id-14 sbs-stn sbp-l sbis-cb cf"><div class="speech-person"><figure class="speech-icon"><img class="speech-icon-image" src="/a.png" alt="わいひら"></figure><div class="speech-name">わいひら</div></div><div class="speech-balloon"><p>コクーンの言葉です。</p></div></div>"#,
        ),
        (
            "SWELL",
            "了",
            "スウェルの言葉です。",
            r#"<div class="swell-block-balloon"><div class="c-balloon -bln-right" data-col="blue"><div class="c-balloon__icon -circle"><img src="/a.png" alt="" class="c-balloon__iconImg" width="80px" height="80px"><span class="c-balloon__iconName">了</span></div><div class="c-balloon__body -speaking -border-none"><div class="c-balloon__text"><p>スウェルの言葉です。</p><span class="c-balloon__shapes"><span class="c-balloon__before"></span><span class="c-balloon__after"></span></span></div></div></div></div>"#,
        ),
        (
            "SANGO",
            "サルワカくん",
            "サンゴの言葉です。",
            r#"<div class="say right"> <div class="chatting"><div class="sc">サンゴの言葉です。</div></div> <p class="faceicon"><img src="/a.png" width="110"><span>サルワカくん</span></p> </div>"#,
        ),
        (
            "AFFINGER",
            "じい",
            "アフィンガーの言葉です。",
            r#"<div class="st-kaiwa-box clearfix st-kaiwa st-kaiwa-1742 wp-block-st-blocks-st-kaiwa"><div class="st-kaiwa-area2"><div class="st-kaiwa-hukidashi2"><div class="st-kaiwa-hukidashi-content"><p>アフィンガーの言葉です。</p></div></div></div><div class="st-kaiwa-face2"><img width="60" height="60" src="/a.png" alt=""><div class="st-kaiwa-face-name">じい</div></div></div>"#,
        ),
        (
            "JIN",
            "ひつじくん",
            "ジンの言葉です。",
            r#"<div class="balloon-box balloon-right balloon-gray balloon-bg-gray clearfix"><div class="balloon-icon "><img src="/a.png" alt="ひつじくん" width="80" height="80"></div><div class="icon-name">ひつじくん</div><div class="balloon-serif"><div class="balloon-content">ジンの言葉です。</div></div></div>"#,
        ),
    ];

    #[test]
    fn keeps_talk_balloons() {
        let para = "これは記事の本文の段落です。ページの中で、いちばん文章が多いところが記事として選ばれます。".repeat(3);
        for (theme, name, words, balloon) in BALLOONS {
            let html = page(&format!(
                "<article><p>{para}</p>{balloon}<p>{para}</p></article>"
            ));
            let out = extract(&html, "https://example.com/news/1").expect("an article");
            let out = crate::sanitize::article_html(&out, Some("https://example.com/news/1"));
            assert!(out.contains(r#"<div class="fa-talk"#), "{theme}: {out}");
            assert!(out.contains(name) && out.contains(words), "{theme}: {out}");
            // The image is in the icon: ours, or OPENCAGE's bare figure.
            assert!(
                out.contains(r#"class="fa-talk-icon"#) || out.contains("<figure><img"),
                "{theme}: {out}"
            );
            // A name outside the icon (JIN) keeps ours, or it would be
            // laid out as the text.
            if *theme == "JIN" {
                assert!(out.contains(r#"class="fa-talk-name""#), "{theme}: {out}");
            }
            // Every sample is on the right but Cocoon's.
            assert_eq!(
                out.contains("fa-talk-r"),
                *theme != "Cocoon",
                "{theme}: {out}"
            );
        }
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
