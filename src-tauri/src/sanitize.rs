//! An article's HTML made safe to put into the window.
//!
//! The HTML comes from whoever runs the feed. ammonia keeps formatting,
//! links and images and drops scripts, event handlers, styles, forms and
//! frames; the CSP (tauri.conf.json) would stop scripts anyway, this is the
//! first of the two walls. Relative links resolve against the article's URL.
//!
//! Images keep their `src` here. The reader never lets the WebView load them
//! (the CSP allows no remote images): it parses this HTML into an inert
//! document, moves `src` aside and asks Rust for each image (fetch.rs
//! `image_data_url`). See src/components/Reader.tsx.

use ammonia::{Builder, UrlRelative};
use std::collections::HashSet;

pub fn article_html(html: &str, base: Option<&str>) -> String {
    let mut b = Builder::default();
    b.url_schemes(HashSet::from(["http", "https", "mailto"]));
    b.link_rel(Some("noopener noreferrer"));
    // srcset would name more images than the one the reader fetches.
    b.rm_tag_attributes("img", &["srcset"]);
    // Inline sizes from the feed fight the reader's own layout.
    b.rm_tag_attributes("img", &["width", "height"]);
    b.add_tag_attributes("a", &["title"]);
    b.add_tags(&["figure", "figcaption", "picture"]);
    match base.and_then(|u| url::Url::parse(u).ok()) {
        Some(base) => b.url_relative(UrlRelative::RewriteWithBase(base)),
        None => b.url_relative(UrlRelative::Deny),
    };
    b.clean(html).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_scripts_and_handlers() {
        let out = article_html(
            r#"<p onclick="x()">hi<script>alert(1)</script></p><iframe src="https://e.x"></iframe><a href="javascript:alert(1)">j</a><style>p{}</style><form><input></form>"#,
            None,
        );
        assert!(!out.contains("script"), "{out}");
        assert!(!out.contains("onclick"), "{out}");
        assert!(!out.contains("iframe"), "{out}");
        assert!(!out.contains("javascript"), "{out}");
        assert!(!out.contains("style"), "{out}");
        assert!(!out.contains("input"), "{out}");
        assert!(out.contains("<p>hi</p>"), "{out}");
    }

    #[test]
    fn resolves_relative_links_and_marks_them() {
        let out = article_html(
            r#"<a href="/post">p</a><img src="img/a.png" srcset="b.png 2x" width="900">"#,
            Some("https://blog.example.com/2025/10/entry"),
        );
        assert!(
            out.contains(r#"href="https://blog.example.com/post""#),
            "{out}"
        );
        assert!(out.contains(r#"rel="noopener noreferrer""#), "{out}");
        assert!(
            out.contains(r#"src="https://blog.example.com/2025/10/img/a.png""#),
            "{out}"
        );
        assert!(!out.contains("srcset") && !out.contains("width"), "{out}");
    }

    #[test]
    fn without_a_base_relative_links_are_dropped() {
        let out = article_html(r#"<a href="/post">p</a>"#, None);
        assert!(!out.contains("href"), "{out}");
    }
}
