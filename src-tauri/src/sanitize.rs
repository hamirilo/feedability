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
//!
//! The feed's classes are dropped, with one exception: the talk balloons of
//! Japanese blogs (an icon, a name and what they say), which only read right
//! laid out as balloons. Their classes become ours (`talk_class`) and
//! src/index.css draws them.

use ammonia::{Builder, UrlRelative};
use std::borrow::Cow;
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
    b.add_tag_attributes("div", &["class"]);
    b.add_tag_attributes("p", &["class"]);
    b.add_tag_attributes("figure", &["class"]);
    b.add_tag_attributes("figcaption", &["class"]);
    b.attribute_filter(|el, attr, value| match (el, attr) {
        ("div" | "p" | "figure" | "figcaption", "class") => talk_class(value).map(Cow::Owned),
        _ => Some(Cow::Borrowed(value)),
    });
    match base.and_then(|u| url::Url::parse(u).ok()) {
        Some(base) => b.url_relative(UrlRelative::RewriteWithBase(base)),
        None => b.url_relative(UrlRelative::Deny),
    };
    b.clean(html).to_string()
}

/// The parts of a talk balloon, in the classes the blog themes write: the
/// box around it, the icon (an image, with the name under it in most
/// themes), the name when it sits outside the icon (JIN), and the text.
/// A class is (theme's word, our class).
const TALK: &[(&str, &str)] = &[
    // OPENCAGE (STORK, Hummingbird, Albatros): `div.voice.l|r` > `figure.icon`
    // (img, figcaption) + `div.voicecomment`. The bare figure is the icon.
    ("voice", "fa-talk"),
    ("voicecomment", "fa-talk-text"),
    // Cocoon: `div.speech-wrap.sbp-l|r` > `div.speech-person` (figure, div
    // name) + `div.speech-balloon`.
    ("speech-wrap", "fa-talk"),
    ("speech-person", "fa-talk-icon"),
    ("speech-balloon", "fa-talk-text"),
    // SWELL: `div.c-balloon.-bln-left|right` > `div.c-balloon__icon` (img,
    // span name) + `div.c-balloon__body`.
    ("c-balloon", "fa-talk"),
    ("c-balloon__icon", "fa-talk-icon"),
    ("c-balloon__body", "fa-talk-text"),
    // SANGO: `div.say.left|right` > `p.faceicon` (img, span name) +
    // `div.chatting`, the face after the text when on the right.
    ("faceicon", "fa-talk-icon"),
    ("chatting", "fa-talk-text"),
    // AFFINGER: `div.st-kaiwa-box` > `div.st-kaiwa-face` (img, div name) +
    // `div.st-kaiwa-area`; on the right the box is the same and its parts
    // end in 2, the face after the text.
    ("st-kaiwa-box", "fa-talk"),
    ("st-kaiwa-face", "fa-talk-icon"),
    ("st-kaiwa-face2", "fa-talk-icon fa-talk-r"),
    ("st-kaiwa-area", "fa-talk-text"),
    ("st-kaiwa-area2", "fa-talk-text"),
    // JIN: `div.balloon-box.balloon-left|right` > `div.balloon-icon` (img) +
    // `div.icon-name` + `div.balloon-serif`.
    ("balloon-box", "fa-talk"),
    ("balloon-icon", "fa-talk-icon"),
    ("icon-name", "fa-talk-name"),
    ("balloon-serif", "fa-talk-text"),
];

/// The classes that put a balloon's box on the right, next to its theme's
/// word for the box.
const TALK_RIGHT: &[(&str, &str)] = &[
    ("voice", "r"),
    ("speech-wrap", "sbp-r"),
    ("c-balloon", "-bln-right"),
    ("say", "right"),
    ("balloon-box", "balloon-right"),
];

/// Our classes for a part of a talk balloon (see `TALK`), from the class the
/// feed or page gave it; None for anything else, whose class is dropped.
/// Ours pass through unchanged, since `fulltext::extract` has already put
/// them in. src/index.css lays the parts out.
pub fn talk_class(class: &str) -> Option<String> {
    let words: Vec<&str> = class.split_ascii_whitespace().collect();
    let has = |w: &str| words.contains(&w);
    if words.iter().all(|w| w.starts_with("fa-talk")) {
        return (!words.is_empty()).then(|| words.join(" "));
    }
    // SANGO's `say` is too common a word on its own.
    let sango = has("say") && (has("left") || has("right"));
    let ours = if sango {
        "fa-talk"
    } else {
        TALK.iter().find(|(theirs, _)| has(theirs))?.1
    };
    let right = ours == "fa-talk" && TALK_RIGHT.iter().any(|(bx, side)| has(bx) && has(side));
    Some(if right {
        format!("{ours} fa-talk-r")
    } else {
        ours.into()
    })
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

    #[test]
    fn talk_balloons_keep_our_classes_only() {
        let out = article_html(
            r#"<div class="voice cf r"><figure class="icon"><img src="https://e.x/a.png"><figcaption class="name">まの</figcaption></figure><div class="voicecomment"><p>こんにちは</p></div></div><div class="note">n</div>"#,
            None,
        );
        assert!(
            out.contains(r#"<div class="fa-talk fa-talk-r"><figure><img"#),
            "{out}"
        );
        assert!(out.contains(r#"<figcaption>まの</figcaption>"#), "{out}");
        assert!(
            out.contains(r#"<div class="fa-talk-text"><p>こんにちは</p>"#),
            "{out}"
        );
        assert!(out.contains("<div>n</div>"), "{out}");
        assert_eq!(talk_class("voice cf l").as_deref(), Some("fa-talk"));
    }

    #[test]
    fn talk_classes_of_each_theme() {
        let cases = [
            (
                "speech-wrap sb-id-25 sbs-stn sbp-r sbis-cb cf",
                "fa-talk fa-talk-r",
            ),
            ("speech-person", "fa-talk-icon"),
            ("speech-balloon", "fa-talk-text"),
            ("c-balloon -bln-left", "fa-talk"),
            ("c-balloon -bln-right -sp-vrtcl", "fa-talk fa-talk-r"),
            ("c-balloon__icon -circle", "fa-talk-icon"),
            ("c-balloon__body -speaking -border-on", "fa-talk-text"),
            ("say right", "fa-talk fa-talk-r"),
            ("faceicon", "fa-talk-icon"),
            ("chatting", "fa-talk-text"),
            (
                "st-kaiwa-box clearfix wp-block-st-blocks-st-kaiwa kaiwaicon2",
                "fa-talk",
            ),
            ("st-kaiwa-face2", "fa-talk-icon fa-talk-r"),
            ("st-kaiwa-area2", "fa-talk-text"),
            (
                "balloon-box balloon-right balloon-gray balloon-bg-gray clearfix",
                "fa-talk fa-talk-r",
            ),
            ("balloon-icon ", "fa-talk-icon"),
            ("icon-name", "fa-talk-name"),
            ("balloon-serif", "fa-talk-text"),
            // Ours, already put in by fulltext::extract.
            ("fa-talk fa-talk-r", "fa-talk fa-talk-r"),
            ("fa-talk-icon", "fa-talk-icon"),
        ];
        for (theirs, ours) in cases {
            assert_eq!(talk_class(theirs).as_deref(), Some(ours), "{theirs}");
        }
        for other in ["say", "note r", "icon", "", "fa-talk evil"] {
            assert_eq!(talk_class(other), None, "{other}");
        }
    }
}
