//! An article as one Markdown file, for keeping it in stockroom
//! (docs/STOCKROOM.md): copied to the clipboard, or sent to stockroom.
//!
//! The body comes from the HTML the reader shows, already through
//! `sanitize::article_html` (scripts dropped, relative links resolved).
//! Images become links: stockroom's window doesn't load outside images.

use htmd::{Element, HtmlToMarkdown};

pub struct Meta<'a> {
    pub title: &'a str,
    pub url: Option<&'a str>,
    pub feed_title: &'a str,
    /// Milliseconds since the epoch.
    pub published: i64,
    pub saved: i64,
    /// The user's offset from UTC in minutes (east positive), for the dates.
    pub utc_offset_minutes: i32,
}

/// The file's name and its text.
pub fn article(meta: &Meta, html: &str) -> (String, String) {
    let date = |ms: i64| ymd(ms + i64::from(meta.utc_offset_minutes) * 60_000);
    let title = meta.title.trim();
    let mut out = format!("# {}\n\n", if title.is_empty() { "記事" } else { title });
    if let Some(url) = meta.url {
        out += &format!("- 元のページ：{url}\n");
    }
    out += &format!(
        "- フィード：{}\n- 公開：{}\n- 保存：{}（Feedability から）\n\n---\n\n",
        meta.feed_title,
        date(meta.published),
        date(meta.saved)
    );
    out += body(html).trim();
    out.push('\n');
    (file_name(title), out)
}

fn body(html: &str) -> String {
    let converter = HtmlToMarkdown::builder()
        .add_handler(
            vec!["img"],
            |_: &dyn htmd::element_handler::Handlers, e: Element| {
                let src = e
                    .attrs
                    .iter()
                    .find(|a| &*a.name.local == "src")
                    .map(|a| a.value.to_string())
                    .filter(|s| s.starts_with("http://") || s.starts_with("https://"))?;
                Some(format!("[画像]({src})").into())
            },
        )
        .build();
    // The HTML is ammonia's output: it parses. A failure keeps the text out
    // rather than the whole Markdown.
    converter.convert(html).unwrap_or_default()
}

/// The title as a file name: characters no file system takes become `_`,
/// at most 100 characters, `記事.md` when there is no title.
pub fn file_name(title: &str) -> String {
    let name: String = title
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(100)
        .collect();
    let name = name.trim().trim_end_matches('.');
    format!("{}.md", if name.is_empty() { "記事" } else { name })
}

/// `2026-10-17` from milliseconds since the epoch (the days-to-civil
/// algorithm, so no date crate is needed for one line).
fn ymd(ms: i64) -> String {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-17 23:30 UTC: 2026-10-18 in Japan.
    const AT: i64 = 1_792_279_800_000;

    fn meta(title: &str) -> Meta<'_> {
        Meta {
            title,
            url: Some("https://example.com/post/1"),
            feed_title: "Example Blog",
            published: AT,
            saved: AT,
            utc_offset_minutes: 540,
        }
    }

    #[test]
    fn writes_the_header_and_the_body() {
        let html = r#"<p>本文の<a href="https://example.com/a">リンク</a>。</p><ul><li>一<ul><li>二</li></ul></li></ul><table><thead><tr><th>A</th></tr></thead><tbody><tr><td>1</td></tr></tbody></table>"#;
        let (name, md) = article(&meta("記事のタイトル"), html);
        assert_eq!(name, "記事のタイトル.md");
        assert!(
            md.starts_with(
                "# 記事のタイトル\n\n- 元のページ：https://example.com/post/1\n- フィード：Example Blog\n- 公開：2026-10-18\n- 保存：2026-10-18（Feedability から）\n\n---\n\n"
            ),
            "{md}"
        );
        assert!(md.contains("[リンク](https://example.com/a)"), "{md}");
        // The nested item stays nested.
        assert!(md.contains("*   一\n    *   二"), "{md}");
        assert!(md.contains("| A |"), "{md}");
        assert!(md.ends_with('\n') && !md.ends_with("\n\n"), "{md:?}");
    }

    #[test]
    fn images_become_links() {
        let (_, md) = article(
            &meta("t"),
            r#"<p>前</p><p><img src="https://example.com/a.png" alt="写真"></p><img src="data:image/png;base64,AAAA">"#,
        );
        assert!(md.contains("[画像](https://example.com/a.png)"), "{md}");
        assert!(!md.contains("!["), "{md}");
        assert!(!md.contains("data:"), "{md}");
    }

    #[test]
    fn dates_follow_the_offset() {
        let mut m = meta("t");
        m.utc_offset_minutes = 0;
        assert!(article(&m, "").1.contains("公開：2026-10-17"));
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(951_782_400_000), "2000-02-29");
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("a/b:c*d?\"e<f>g|h\\i"), "a_b_c_d__e_f_g_h_i.md");
        assert_eq!(file_name("  "), "記事.md");
        assert_eq!(file_name("改行\nあり"), "改行_あり.md");
        assert_eq!(
            file_name(&"あ".repeat(150)),
            format!("{}.md", "あ".repeat(100))
        );
        assert_eq!(file_name("..."), "記事.md");
    }
}
