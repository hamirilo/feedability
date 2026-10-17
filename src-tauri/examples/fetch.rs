//! Fetches a feed (or a page that names one) the way the app does and prints
//! what it would store, without the app:
//!
//!     cargo run --manifest-path src-tauri/Cargo.toml --example fetch -- https://example.com/

use feedability_lib::{feed, fetch};

fn main() {
    let Some(input) = std::env::args().nth(1) else {
        eprintln!("usage: fetch <url>");
        std::process::exit(2);
    };
    let result = tauri::async_runtime::block_on(async {
        let url = fetch::normalize_url(&input)?;
        let res =
            match fetch::get(&url, None, None, fetch::FEED_ACCEPT, fetch::MAX_FEED_BYTES).await? {
                fetch::Outcome::Fetched(r) => r,
                fetch::Outcome::NotModified => return Err("304".to_string()),
            };
        println!(
            "{} ({} bytes, {:?})",
            res.url,
            res.body.len(),
            res.content_type
        );
        match feed::parse(&res.body, &res.url) {
            Ok(f) => Ok(f),
            Err(e) if feed::looks_like_html(&res.body) => {
                let found = feed::discover(&res.body, &res.url);
                println!("a page; its feeds: {found:?}");
                let first = found.first().ok_or(e)?;
                match fetch::get(first, None, None, fetch::FEED_ACCEPT, fetch::MAX_FEED_BYTES)
                    .await?
                {
                    fetch::Outcome::Fetched(r) => feed::parse(&r.body, &r.url),
                    fetch::Outcome::NotModified => Err("304".to_string()),
                }
            }
            Err(e) => Err(e),
        }
    });
    match result {
        Ok(f) => {
            println!(
                "title: {:?}\nsite: {:?}\narticles: {}",
                f.title,
                f.site_url,
                f.articles.len()
            );
            for a in f.articles.iter().take(5) {
                println!(
                    "- {} <{}> {:?}\n  {}",
                    a.title,
                    a.guid,
                    a.url,
                    a.summary.chars().take(80).collect::<String>()
                );
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
