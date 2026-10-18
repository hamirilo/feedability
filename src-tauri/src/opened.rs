//! What the OS hands the app to open (docs/OPEN_WITH.md): a `feed:` link
//! (subscribe, the add-feed dialog filled in) or an `.opml` file (import,
//! after asking). Both come from outside, so nothing happens without the
//! user: the URL only fills a field, the file is only read once they agree.
//!
//! - macOS: both arrive as URLs (`feed:…`, `file://…`) through the
//!   deep-link plugin, at launch or while running.
//! - Windows: as the command line, of this launch or of a second one
//!   (single-instance hands its arguments over).

use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum OpenRequest {
    /// The feed's URL, checked (`fetch::normalize_url`).
    Feed {
        url: String,
    },
    Opml {
        path: String,
    },
}

/// A `feed:` link as the feed's URL: `feed://host/path` is http,
/// `feed:https://…` is what follows. None when it isn't one, or what it
/// names isn't an http(s) URL.
pub fn feed_link(link: &str) -> Option<String> {
    let link = link.trim();
    let rest = link
        .get(..5)
        .filter(|s| s.eq_ignore_ascii_case("feed:"))
        .map(|_| &link[5..])?;
    let target = match rest.strip_prefix("//") {
        // feed://https://… (seen in the wild) is the inner URL.
        Some(inner) if inner.contains("://") => inner.to_string(),
        Some(host_path) => format!("http://{host_path}"),
        None => rest.to_string(),
    };
    if target.is_empty() || !target.contains("://") {
        return None;
    }
    crate::fetch::normalize_url(&target).ok()
}

fn is_opml(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("opml"))
}

/// One thing the OS handed over: a URL (macOS), or a command-line argument
/// (Windows: a link or a file's path).
pub fn classify(arg: &str) -> Option<OpenRequest> {
    if let Some(url) = feed_link(arg) {
        return Some(OpenRequest::Feed { url });
    }
    let path = match url::Url::parse(arg) {
        Ok(u) if u.scheme() == "file" => u.to_file_path().ok()?,
        // A drive letter parses as a scheme ("c:"); anything else is a URL
        // that isn't ours.
        Ok(u) if u.scheme().len() > 1 => return None,
        _ => PathBuf::from(arg),
    };
    (is_opml(&path) && path.is_file()).then(|| OpenRequest::Opml {
        path: path.to_string_lossy().into_owned(),
    })
}

/// The command line of a launch, the program's own name left out.
pub fn from_args<S: AsRef<str>>(args: &[S]) -> Vec<OpenRequest> {
    args.iter()
        .skip(1)
        .filter_map(|a| classify(a.as_ref()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_feed_links() {
        assert_eq!(
            feed_link("feed:https://example.com/rss.xml").as_deref(),
            Some("https://example.com/rss.xml")
        );
        assert_eq!(
            feed_link("feed://example.com/atom").as_deref(),
            Some("http://example.com/atom")
        );
        assert_eq!(
            feed_link("FEED:http://example.com/").as_deref(),
            Some("http://example.com/")
        );
        assert_eq!(
            feed_link("feed://https://example.com/rss").as_deref(),
            Some("https://example.com/rss")
        );
        assert_eq!(feed_link("feed:javascript:alert(1)"), None);
        assert_eq!(feed_link("feed:file:///etc/passwd"), None);
        assert_eq!(feed_link("feed:"), None);
        assert_eq!(feed_link("https://example.com/rss"), None);
    }

    #[test]
    fn tells_links_files_and_the_rest() {
        let dir = std::env::temp_dir().join(format!("feedability-opened-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("subs.OPML");
        std::fs::write(&file, "<opml/>").unwrap();
        let path = file.to_string_lossy().into_owned();
        let url = url::Url::from_file_path(&file).unwrap().to_string();

        assert_eq!(
            classify(&path),
            Some(OpenRequest::Opml { path: path.clone() })
        );
        assert_eq!(
            classify(&url),
            Some(OpenRequest::Opml { path: path.clone() })
        );
        assert_eq!(
            classify("feed:https://example.com/rss"),
            Some(OpenRequest::Feed {
                url: "https://example.com/rss".into()
            })
        );
        // Not an OPML file, not there, not ours.
        let other = dir.join("notes.txt");
        std::fs::write(&other, "x").unwrap();
        assert_eq!(classify(&other.to_string_lossy()), None);
        assert_eq!(classify(&dir.join("gone.opml").to_string_lossy()), None);
        assert_eq!(classify("https://example.com/a.opml"), None);
        assert_eq!(classify("--flag"), None);

        // The program's name is not something to open.
        assert_eq!(
            from_args(&[path.as_str(), "feed:https://example.com/rss"]),
            vec![OpenRequest::Feed {
                url: "https://example.com/rss".into()
            }]
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
