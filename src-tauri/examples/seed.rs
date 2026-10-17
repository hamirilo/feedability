//! Test data: feeds with many articles in a feedability.db, to check that
//! the list stays smooth (desktop-platform Architecture §9: 10,000 articles).
//! Quit the app first, then:
//!
//!     cargo run --release --manifest-path src-tauri/Cargo.toml --example seed -- <path to feedability.db> [articles]
//!
//! The database is in the app's data folder (README「データ」). It adds to
//! what is there; it prints how long the list's queries take afterwards.

use feedability_lib::db::{self, ArticleQuery, NewArticle, View};
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: seed <feedability.db> [articles]");
        std::process::exit(2);
    };
    let total: usize = args.next().and_then(|n| n.parse().ok()).unwrap_or(10_000);
    let mut conn = db::open(std::path::Path::new(&path)).expect("open");
    let feeds = 50;
    let now = db::now_ms();
    let stamp = now;
    for f in 0..feeds {
        let folder = db::folder_named(&conn, &format!("フォルダ {}", f % 5)).unwrap();
        let url = format!("https://seed.example/{stamp}/{f}/feed.xml");
        let id = db::add_feed(
            &conn,
            &url,
            &format!("テストのフィード {f}"),
            None,
            Some(folder),
        )
        .unwrap();
        let articles: Vec<NewArticle> = (0..total / feeds)
            .map(|i| NewArticle {
                guid: format!("{f}-{i}"),
                url: Some(format!("https://seed.example/{f}/{i}")),
                title: format!("フィード {f} の記事 {i}"),
                author: None,
                summary: "テストの要約。".repeat(20),
                content: "<p>テストの本文。</p>".repeat(30),
                published: Some(now - (i * feeds + f) as i64 * 60_000),
                // Two in three with a picture, for the 画像付き layout.
                thumb_url: (i % 3 != 0).then(|| format!("https://seed.example/{f}/{i}.jpg")),
            })
            .collect();
        db::store_articles(&mut conn, id, &articles).unwrap();
    }
    let time = |label: &str, q: ArticleQuery| {
        let t = Instant::now();
        let n = db::query_articles(&conn, &q).unwrap().len();
        println!("{label}: {n} rows in {:?}", t.elapsed());
    };
    let q = |view, unread_only, search: &str| ArticleQuery {
        view,
        unread_only,
        search: search.into(),
        keep: None,
        oldest_first: false,
    };
    time("all", q(View::All, false, ""));
    time("unread", q(View::All, true, ""));
    time("folder", q(View::Folder { id: 1 }, false, ""));
    time("search", q(View::All, false, "記事 12"));
    let t = Instant::now();
    db::sidebar(&conn).unwrap();
    println!("sidebar in {:?}", t.elapsed());
    db::close(conn);
}
