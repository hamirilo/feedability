//! OPML: the subscription list other readers import and export. Folders are
//! one level deep here; deeper outlines go into their top folder.

use crate::db::OpmlFeed;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

/// An OPML file bigger than this is not a subscription list.
pub const MAX_BYTES: u64 = 5 * 1024 * 1024;

pub fn parse(xml: &str) -> Result<Vec<OpmlFeed>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut feeds = Vec::new();
    // The outlines we are inside of: Some(name) for a folder.
    let mut stack: Vec<Option<String>> = Vec::new();
    let mut saw_opml = false;
    loop {
        let event = reader
            .read_event()
            .map_err(|e| format!("OPML として読めませんでした（{e}）"))?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(event, Event::Empty(_));
                match e.local_name().as_ref() {
                    "opml" => saw_opml = true,
                    "outline" => {
                        let attr = |k: &str| attr(e, k);
                        let name = attr("title")
                            .or_else(|| attr("text"))
                            .filter(|s| !s.trim().is_empty());
                        match attr("xmlUrl")
                            .filter(|u| u.starts_with("http://") || u.starts_with("https://"))
                        {
                            Some(url) => feeds.push(OpmlFeed {
                                title: name.unwrap_or_else(|| url.clone()),
                                url,
                                site_url: attr("htmlUrl").filter(|u| !u.is_empty()),
                                folder: stack.iter().flatten().next().cloned(),
                            }),
                            None if !empty => stack.push(name),
                            None => {}
                        }
                        // A feed outline with children: its children still need a closing pop.
                        if !empty && attr("xmlUrl").is_some() {
                            stack.push(None);
                        }
                    }
                    _ => {}
                }
            }
            Event::End(ref e) if e.local_name().into_inner() == "outline" => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !saw_opml {
        return Err("OPML ファイルではありません".into());
    }
    Ok(feeds)
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|v| v.trim().to_string())
}

fn esc(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}

/// Feeds in folder order (`db::feeds_for_export`): top level first.
pub fn write(feeds: &[OpmlFeed]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head><title>Feedability</title></head>\n  <body>\n",
    );
    let line = |f: &OpmlFeed, indent: &str| {
        let site = f
            .site_url
            .as_deref()
            .map(|u| format!(" htmlUrl=\"{}\"", esc(u)))
            .unwrap_or_default();
        format!(
            "{indent}<outline type=\"rss\" text=\"{t}\" title=\"{t}\" xmlUrl=\"{u}\"{site}/>\n",
            t = esc(&f.title),
            u = esc(&f.url)
        )
    };
    for f in feeds.iter().filter(|f| f.folder.is_none()) {
        out.push_str(&line(f, "    "));
    }
    let mut folders: Vec<&str> = feeds.iter().filter_map(|f| f.folder.as_deref()).collect();
    folders.dedup();
    for name in folders {
        out.push_str(&format!(
            "    <outline text=\"{n}\" title=\"{n}\">\n",
            n = esc(name)
        ));
        for f in feeds.iter().filter(|f| f.folder.as_deref() == Some(name)) {
            out.push_str(&line(f, "      "));
        }
        out.push_str("    </outline>\n");
    }
    out.push_str("  </body>\n</opml>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_folders_and_feeds() {
        let xml = r#"<?xml version="1.0"?>
<opml version="1.0"><head><title>x</title></head><body>
  <outline text="Top" type="rss" xmlUrl="https://top.example/feed" htmlUrl="https://top.example/"/>
  <outline text="技術">
    <outline title="A &amp; B" text="ignored" xmlUrl="https://a.example/rss"/>
    <outline text="入れ子">
      <outline text="Deep" xmlUrl="https://deep.example/rss"/>
    </outline>
  </outline>
  <outline text="no url"/>
  <outline text="bad" xmlUrl="file:///etc/passwd"/>
</body></opml>"#;
        let feeds = parse(xml).unwrap();
        let got: Vec<(&str, &str, Option<&str>)> = feeds
            .iter()
            .map(|f| (f.title.as_str(), f.url.as_str(), f.folder.as_deref()))
            .collect();
        assert_eq!(
            got,
            [
                ("Top", "https://top.example/feed", None),
                ("A & B", "https://a.example/rss", Some("技術")),
                ("Deep", "https://deep.example/rss", Some("技術")),
            ]
        );
        assert_eq!(feeds[0].site_url.as_deref(), Some("https://top.example/"));
    }

    #[test]
    fn round_trips() {
        let feeds = vec![
            OpmlFeed {
                url: "https://a.example/rss?x=1&y=2".into(),
                title: "<A>".into(),
                site_url: None,
                folder: None,
            },
            OpmlFeed {
                url: "https://b.example/rss".into(),
                title: "B \"quoted\"".into(),
                site_url: Some("https://b.example/".into()),
                folder: Some("ニュース & 技術".into()),
            },
        ];
        let back = parse(&write(&feeds)).unwrap();
        assert_eq!(back.len(), 2);
        for (a, b) in feeds.iter().zip(&back) {
            assert_eq!(
                (&a.url, &a.title, &a.site_url, &a.folder),
                (&b.url, &b.title, &b.site_url, &b.folder)
            );
        }
    }

    #[test]
    fn rejects_other_files() {
        assert!(parse("<rss></rss>").is_err());
        assert!(parse("not xml <").is_err());
    }
}
