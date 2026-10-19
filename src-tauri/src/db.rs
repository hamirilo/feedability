//! The subscriptions and articles: one SQLite file in the app's data folder
//! (feedability.db, see README「データ」). Everything here takes a plain
//! `Connection` so the tests run on an in-memory database.
//!
//! An article keeps `seen_at`, the last refresh that still found it in its
//! feed. Old read articles are removed only once their feed has dropped them
//! too (`purge`): removing one the feed still lists would bring it back as
//! unread on the next refresh.

use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub type DbResult<T> = rusqlite::Result<T>;

/// The schema version in `PRAGMA user_version`. Add to `migrate` in steps;
/// an older app opening a newer file must still be able to read it.
const VERSION: i64 = 8;

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn open(path: &Path) -> DbResult<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    setup(&conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> DbResult<Connection> {
    let conn = Connection::open_in_memory()?;
    setup(&conn)?;
    Ok(conn)
}

fn setup(conn: &Connection) -> DbResult<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(conn)?;
    let indexed = index_missing(conn)?;
    if indexed > 0 {
        log::info!("indexed {indexed} articles for the search");
    }
    Ok(())
}

/// Everything written back into feedability.db and the WAL emptied, so the
/// file is complete on its own (a backup copied while the app is closed).
pub fn close(conn: Connection) {
    if let Err(e) = conn.pragma_update(None, "wal_checkpoint", "TRUNCATE") {
        log::warn!("checkpoint: {e}");
    }
    if let Err((_, e)) = conn.close() {
        log::warn!("close: {e}");
    }
}

fn migrate(conn: &Connection) -> DbResult<()> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS folders (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS feeds (
               id INTEGER PRIMARY KEY,
               url TEXT NOT NULL UNIQUE,
               -- From the feed itself; `custom_title` when the user renamed it.
               title TEXT NOT NULL,
               custom_title TEXT,
               site_url TEXT,
               folder_id INTEGER REFERENCES folders(id) ON DELETE SET NULL,
               added_at INTEGER NOT NULL,
               fetched_at INTEGER,
               -- Why the last refresh failed; NULL when it worked.
               error TEXT,
               -- For conditional requests: an unchanged feed costs one 304.
               etag TEXT,
               last_modified TEXT
             );
             CREATE TABLE IF NOT EXISTS articles (
               id INTEGER PRIMARY KEY,
               feed_id INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
               guid TEXT NOT NULL,
               url TEXT,
               title TEXT NOT NULL,
               author TEXT,
               -- Plain text for the list; `content` is the feed's HTML as is,
               -- sanitized when shown (sanitize.rs).
               summary TEXT NOT NULL DEFAULT '',
               content TEXT NOT NULL DEFAULT '',
               published INTEGER NOT NULL,
               added_at INTEGER NOT NULL,
               seen_at INTEGER NOT NULL,
               read INTEGER NOT NULL DEFAULT 0,
               starred INTEGER NOT NULL DEFAULT 0,
               UNIQUE (feed_id, guid)
             );
             CREATE INDEX IF NOT EXISTS articles_published ON articles(published DESC);
             CREATE INDEX IF NOT EXISTS articles_feed ON articles(feed_id, published DESC);
             CREATE INDEX IF NOT EXISTS articles_unread ON articles(feed_id) WHERE read = 0;
             CREATE INDEX IF NOT EXISTS articles_starred ON articles(starred) WHERE starred = 1;
             COMMIT;",
        )?;
    }
    if version < 2 {
        // あとで読む (docs/LATER.md): `later_at` is when it went in, for the
        // queue's order; NULL when not in it. An older app ignores the column.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE articles ADD COLUMN later_at INTEGER;
             CREATE INDEX IF NOT EXISTS articles_later ON articles(later_at) WHERE later_at IS NOT NULL;
             COMMIT;",
        )?;
    }
    if version < 3 {
        // Feed icons (docs/FAVICON.md): `icon` is a data: URL; `icon_checked_at`
        // is when it was last looked for (NULL: never), so a site without one
        // isn't asked at every refresh; `icon_url` / `logo_url` are the feed's
        // own declared icon and logo from its last parse. An older app ignores them.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE feeds ADD COLUMN icon TEXT;
             ALTER TABLE feeds ADD COLUMN icon_checked_at INTEGER;
             ALTER TABLE feeds ADD COLUMN icon_url TEXT;
             ALTER TABLE feeds ADD COLUMN logo_url TEXT;
             COMMIT;",
        )?;
    }
    if version < 4 {
        // The list's 画像付き layout (docs/LIST_LAYOUT.md): the URL of the
        // article's picture, found when it is stored; NULL: none. Articles
        // stored before get one at the next refresh that still lists them:
        // the conditional-request headers are cleared so that refresh reads
        // every feed once, where a 304 would skip storing. An older app
        // ignores the column.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE articles ADD COLUMN thumb_url TEXT;
             UPDATE feeds SET etag = NULL, last_modified = NULL;
             COMMIT;",
        )?;
    }
    if version < 5 {
        // 全文取得 (docs/FULL_TEXT.md): `feeds.full_text` is the user's switch
        // per feed. `articles.full_content` is the article's HTML from its
        // page, as extracted (sanitized when shown), and `full_status` where
        // that stands (`FullStatus`). An older app ignores the columns.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE feeds ADD COLUMN full_text INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE articles ADD COLUMN full_content TEXT;
             ALTER TABLE articles ADD COLUMN full_status INTEGER;
             CREATE INDEX IF NOT EXISTS articles_full_queued ON articles(full_status) WHERE full_status = 0;
             COMMIT;",
        )?;
    }
    if version < 6 {
        // Pages saved from the browser extension (docs/EXTENSION.md) belong
        // to one feed of their own, `saved = 1`, which is never refreshed,
        // exported or shown in the sidebar. An older app shows it as a feed
        // whose refresh fails, and nothing else.
        conn.execute_batch(
            "BEGIN;
             ALTER TABLE feeds ADD COLUMN saved INTEGER NOT NULL DEFAULT 0;
             COMMIT;",
        )?;
    }
    if version < 7 {
        // ミュート (docs/MUTE.md): new articles with all of `words` in them
        // are marked read when stored. In one feed, one folder, or all (both
        // NULL). `action` is what a rule does, 'read' the only one so far. An
        // older app ignores the table.
        conn.execute_batch(
            "BEGIN;
             CREATE TABLE IF NOT EXISTS rules (
               id INTEGER PRIMARY KEY,
               words TEXT NOT NULL,
               feed_id INTEGER REFERENCES feeds(id) ON DELETE CASCADE,
               folder_id INTEGER REFERENCES folders(id) ON DELETE CASCADE,
               action TEXT NOT NULL DEFAULT 'read',
               enabled INTEGER NOT NULL DEFAULT 1,
               created_at INTEGER NOT NULL
             );
             COMMIT;",
        )?;
    }
    if version < 8 {
        // The search (docs/SEARCH.md): each article's title, summary and text
        // without the HTML, by trigram so that Japanese, with no spaces
        // between words, is found inside a run of text. Rust fills it
        // (`index_articles`; `index_missing` for the articles already there).
        // An older app's deletes keep it right through the trigger; what it
        // adds is indexed when this app next opens the file.
        conn.execute_batch(
            "BEGIN;
             CREATE VIRTUAL TABLE IF NOT EXISTS articles_fts USING fts5(title, summary, body, tokenize = 'trigram');
             CREATE TRIGGER IF NOT EXISTS articles_fts_delete AFTER DELETE ON articles BEGIN
               DELETE FROM articles_fts WHERE rowid = old.id;
             END;
             COMMIT;",
        )?;
    }
    if version < VERSION {
        conn.pragma_update(None, "user_version", VERSION)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- sidebar

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FolderRow {
    pub id: i64,
    pub name: String,
    pub unread: i64,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FeedRow {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub site_url: Option<String>,
    pub folder_id: Option<i64>,
    pub unread: i64,
    pub error: Option<String>,
    pub fetched_at: Option<i64>,
    /// 全文取得 is on.
    pub full_text: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Sidebar {
    pub folders: Vec<FolderRow>,
    pub feeds: Vec<FeedRow>,
    pub unread: i64,
    pub starred: i64,
    pub later: i64,
}

pub fn sidebar(conn: &Connection) -> DbResult<Sidebar> {
    let mut stmt = conn.prepare(
        "SELECT f.id, COALESCE(f.custom_title, f.title), f.url, f.site_url, f.folder_id, f.error, f.fetched_at,
                (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id AND a.read = 0), f.full_text
         FROM feeds f WHERE f.saved = 0
         ORDER BY COALESCE(f.custom_title, f.title) COLLATE NOCASE, f.id",
    )?;
    let feeds = stmt
        .query_map([], |r| {
            Ok(FeedRow {
                id: r.get(0)?,
                title: r.get(1)?,
                url: r.get(2)?,
                site_url: r.get(3)?,
                folder_id: r.get(4)?,
                error: r.get(5)?,
                fetched_at: r.get(6)?,
                unread: r.get(7)?,
                full_text: r.get(8)?,
            })
        })?
        .collect::<DbResult<Vec<_>>>()?;
    let mut stmt = conn.prepare("SELECT id, name FROM folders ORDER BY name COLLATE NOCASE, id")?;
    let folders = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            Ok(FolderRow {
                id,
                name: r.get(1)?,
                unread: 0,
            })
        })?
        .collect::<DbResult<Vec<_>>>()?
        .into_iter()
        .map(|mut f| {
            f.unread = feeds
                .iter()
                .filter(|x| x.folder_id == Some(f.id))
                .map(|x| x.unread)
                .sum();
            f
        })
        .collect();
    let unread = feeds.iter().map(|f| f.unread).sum();
    let starred = conn.query_row("SELECT COUNT(*) FROM articles WHERE starred = 1", [], |r| {
        r.get(0)
    })?;
    let later = conn.query_row(
        "SELECT COUNT(*) FROM articles WHERE later_at IS NOT NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(Sidebar {
        folders,
        feeds,
        unread,
        starred,
        later,
    })
}

// ---------------------------------------------------------------- folders

pub fn create_folder(conn: &Connection, name: &str) -> DbResult<i64> {
    conn.execute("INSERT INTO folders (name) VALUES (?1)", [name])?;
    Ok(conn.last_insert_rowid())
}

/// The folder with this name, made if there is none (OPML import).
pub fn folder_named(conn: &Connection, name: &str) -> DbResult<i64> {
    let found = conn
        .query_row(
            "SELECT id FROM folders WHERE name = ?1 ORDER BY id LIMIT 1",
            [name],
            |r| r.get(0),
        )
        .optional()?;
    match found {
        Some(id) => Ok(id),
        None => create_folder(conn, name),
    }
}

pub fn rename_folder(conn: &Connection, id: i64, name: &str) -> DbResult<()> {
    conn.execute(
        "UPDATE folders SET name = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    Ok(())
}

/// The folder only: its feeds move to the top level (`ON DELETE SET NULL`).
pub fn delete_folder(conn: &Connection, id: i64) -> DbResult<()> {
    conn.execute("DELETE FROM folders WHERE id = ?1", [id])?;
    Ok(())
}

// ------------------------------------------------------------------ feeds

/// What a refresh needs to ask for a feed.
#[derive(Clone, Debug)]
pub struct FeedTarget {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub fn feed_id_by_url(conn: &Connection, url: &str) -> DbResult<Option<i64>> {
    conn.query_row("SELECT id FROM feeds WHERE url = ?1", [url], |r| r.get(0))
        .optional()
}

pub fn add_feed(
    conn: &Connection,
    url: &str,
    title: &str,
    site_url: Option<&str>,
    folder_id: Option<i64>,
) -> DbResult<i64> {
    conn.execute(
        "INSERT INTO feeds (url, title, site_url, folder_id, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![url, title, site_url, folder_id, now_ms()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Feeds to refresh: `ids`, or all of them.
pub fn feed_targets(conn: &Connection, ids: Option<&[i64]>) -> DbResult<Vec<FeedTarget>> {
    let mut stmt = conn.prepare(
        "SELECT id, url, COALESCE(custom_title, title), etag, last_modified FROM feeds WHERE saved = 0 ORDER BY id",
    )?;
    let all = stmt
        .query_map([], |r| {
            Ok(FeedTarget {
                id: r.get(0)?,
                url: r.get(1)?,
                title: r.get(2)?,
                etag: r.get(3)?,
                last_modified: r.get(4)?,
            })
        })?
        .collect::<DbResult<Vec<_>>>()?;
    Ok(match ids {
        Some(ids) => all.into_iter().filter(|f| ids.contains(&f.id)).collect(),
        None => all,
    })
}

/// An empty `title` keeps the feed's own; the user's name otherwise.
pub fn rename_feed(conn: &Connection, id: i64, title: &str) -> DbResult<()> {
    let custom = (!title.trim().is_empty()).then(|| title.trim());
    conn.execute(
        "UPDATE feeds SET custom_title = ?2 WHERE id = ?1",
        params![id, custom],
    )?;
    Ok(())
}

pub fn move_feed(conn: &Connection, id: i64, folder_id: Option<i64>) -> DbResult<()> {
    conn.execute(
        "UPDATE feeds SET folder_id = ?2 WHERE id = ?1",
        params![id, folder_id],
    )?;
    Ok(())
}

/// The feed and its articles, starred ones included.
pub fn delete_feed(conn: &Connection, id: i64) -> DbResult<()> {
    conn.execute("DELETE FROM feeds WHERE id = ?1", [id])?;
    Ok(())
}

/// After a refresh that reached the server: the feed's own title and site
/// (the user's name stays in `custom_title`), and what to send next time.
pub fn feed_fetched(
    conn: &Connection,
    id: i64,
    title: Option<&str>,
    site_url: Option<&str>,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> DbResult<()> {
    conn.execute(
        "UPDATE feeds SET title = COALESCE(?2, title), site_url = COALESCE(?3, site_url),
                          etag = ?4, last_modified = ?5, fetched_at = ?6, error = NULL
         WHERE id = ?1",
        params![
            id,
            title.filter(|t| !t.trim().is_empty()),
            site_url,
            etag,
            last_modified,
            now_ms()
        ],
    )?;
    Ok(())
}

pub fn feed_failed(conn: &Connection, id: i64, error: &str) -> DbResult<()> {
    conn.execute(
        "UPDATE feeds SET error = ?2, fetched_at = ?3 WHERE id = ?1",
        params![id, error, now_ms()],
    )?;
    Ok(())
}

/// An unchanged feed (304) still lists the same articles: they were seen now.
pub fn feed_unchanged(conn: &Connection, id: i64) -> DbResult<()> {
    let now = now_ms();
    conn.execute(
        "UPDATE feeds SET fetched_at = ?2, error = NULL WHERE id = ?1",
        params![id, now],
    )?;
    conn.execute(
        "UPDATE articles SET seen_at = ?2 WHERE feed_id = ?1",
        params![id, now],
    )?;
    Ok(())
}

/// Everything the feed-info dialog shows (docs/FEED_INFO.md).
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FeedInfo {
    pub id: i64,
    /// The name shown: the user's, else the feed's own.
    pub title: String,
    /// The feed's own title.
    pub original_title: String,
    pub custom_title: Option<String>,
    pub url: String,
    pub site_url: Option<String>,
    pub folder_id: Option<i64>,
    pub added_at: i64,
    pub fetched_at: Option<i64>,
    pub error: Option<String>,
    pub articles: i64,
    pub unread: i64,
    pub full_text: bool,
}

pub fn feed_info(conn: &Connection, id: i64) -> DbResult<Option<FeedInfo>> {
    conn.query_row(
        "SELECT f.id, COALESCE(f.custom_title, f.title), f.title, f.custom_title, f.url, f.site_url,
                f.folder_id, f.added_at, f.fetched_at, f.error,
                (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id),
                (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id AND a.read = 0), f.full_text
         FROM feeds f WHERE f.id = ?1",
        [id],
        |r| {
            Ok(FeedInfo {
                id: r.get(0)?,
                title: r.get(1)?,
                original_title: r.get(2)?,
                custom_title: r.get(3)?,
                url: r.get(4)?,
                site_url: r.get(5)?,
                folder_id: r.get(6)?,
                added_at: r.get(7)?,
                fetched_at: r.get(8)?,
                error: r.get(9)?,
                articles: r.get(10)?,
                unread: r.get(11)?,
                full_text: r.get(12)?,
            })
        },
    )
    .optional()
}

/// Points the feed at a new URL. What belongs to the old address is cleared:
/// the conditional-request headers, the error, the site (the new feed names
/// its own, or has none), the icon and where it came from (it is looked up
/// again). The user's name and the articles stay. Fails
/// on a URL another feed has (`UNIQUE`). False: there is no such feed.
pub fn change_feed_url(conn: &Connection, id: i64, url: &str) -> DbResult<bool> {
    let n = conn.execute(
        "UPDATE feeds SET url = ?2, etag = NULL, last_modified = NULL, error = NULL, site_url = NULL,
                          icon = NULL, icon_checked_at = NULL, icon_url = NULL, logo_url = NULL
         WHERE id = ?1",
        params![id, url],
    )?;
    Ok(n > 0)
}

// ------------------------------------------------------------------ icons

/// A feed to look an icon up for.
#[derive(Clone, Debug)]
pub struct IconTarget {
    pub id: i64,
    pub url: String,
    pub site_url: Option<String>,
    /// The icon and logo the feed declared when last parsed.
    pub icon_url: Option<String>,
    pub logo_url: Option<String>,
}

/// What the feed names as its icon and logo, kept on every successful parse
/// (including none) so the lookup can run later without fetching the feed.
pub fn set_icon_urls(
    conn: &Connection,
    id: i64,
    icon_url: Option<&str>,
    logo_url: Option<&str>,
) -> DbResult<()> {
    conn.execute(
        "UPDATE feeds SET icon_url = ?2, logo_url = ?3 WHERE id = ?1",
        params![id, icon_url, logo_url],
    )?;
    Ok(())
}

/// Feeds never looked up, or not since `older_than_ms`.
pub fn feeds_needing_icon(conn: &Connection, older_than_ms: i64) -> DbResult<Vec<IconTarget>> {
    let mut stmt = conn.prepare(
        "SELECT id, url, site_url, icon_url, logo_url FROM feeds
         WHERE saved = 0 AND (icon_checked_at IS NULL OR icon_checked_at < ?1) ORDER BY id",
    )?;
    let rows = stmt.query_map([older_than_ms], |r| {
        Ok(IconTarget {
            id: r.get(0)?,
            url: r.get(1)?,
            site_url: r.get(2)?,
            icon_url: r.get(3)?,
            logo_url: r.get(4)?,
        })
    })?;
    rows.collect()
}

/// A lookup's result for the feed at `url`. The check is recorded either way;
/// `None` (nothing found, or the site didn't answer) keeps the icon the feed
/// already had. Nothing is written when the feed has moved to another URL
/// meanwhile: the result is the old site's, and the move left the check pending.
pub fn set_icon(conn: &Connection, id: i64, url: &str, icon: Option<&str>) -> DbResult<()> {
    conn.execute(
        "UPDATE feeds SET icon = COALESCE(?3, icon), icon_checked_at = ?4 WHERE id = ?1 AND url = ?2",
        params![id, url, icon, now_ms()],
    )?;
    Ok(())
}

/// Every feed's icon (a data: URL), for the sidebar and the list.
pub fn icons(conn: &Connection) -> DbResult<Vec<(i64, String)>> {
    let mut stmt = conn.prepare("SELECT id, icon FROM feeds WHERE icon IS NOT NULL ORDER BY id")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OpmlFeed {
    pub url: String,
    pub title: String,
    pub site_url: Option<String>,
    pub folder: Option<String>,
}

/// Every feed with its folder's name, for the OPML export.
pub fn feeds_for_export(conn: &Connection) -> DbResult<Vec<OpmlFeed>> {
    let mut stmt = conn.prepare(
        "SELECT f.url, COALESCE(f.custom_title, f.title), f.site_url, d.name
         FROM feeds f LEFT JOIN folders d ON d.id = f.folder_id
         WHERE f.saved = 0
         ORDER BY d.name COLLATE NOCASE, COALESCE(f.custom_title, f.title) COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(OpmlFeed {
            url: r.get(0)?,
            title: r.get(1)?,
            site_url: r.get(2)?,
            folder: r.get(3)?,
        })
    })?;
    rows.collect()
}

// --------------------------------------------------------------- articles

/// An article as a feed lists it (feed.rs).
#[derive(Debug, Clone, PartialEq)]
pub struct NewArticle {
    pub guid: String,
    pub url: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub summary: String,
    pub content: String,
    pub published: Option<i64>,
    /// The picture for the list (feed.rs `thumbnail`).
    pub thumb_url: Option<String>,
}

/// Adds the feed's new articles and updates the ones it already had (a
/// corrected title or text). Read and starred stay as they were, and so does
/// the text from the page (全文取得). New articles of a feed with 全文取得 on
/// are queued for it, unless a ミュート rule marks them read. Returns how many
/// were new.
pub fn store_articles(
    conn: &mut Connection,
    feed_id: i64,
    articles: &[NewArticle],
) -> DbResult<usize> {
    let now = now_ms();
    let tx = conn.transaction()?;
    let mut added = 0;
    {
        let rules = rules_for_feed(&tx, feed_id)?;
        let mut exists = tx.prepare(
            "SELECT id, title, summary, content FROM articles WHERE feed_id = ?1 AND guid = ?2",
        )?;
        let mut insert = tx.prepare(
            "INSERT INTO articles (feed_id, guid, url, title, author, summary, content, published, added_at, seen_at, thumb_url,
                                   read, full_status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9, ?10, ?11,
                     (SELECT CASE WHEN full_text = 1 AND ?11 = 0 THEN 0 END FROM feeds WHERE id = ?1))",
        )?;
        let mut update = tx.prepare(
            "UPDATE articles SET url = ?2, title = ?3, author = ?4, summary = ?5, content = ?6, seen_at = ?7,
                                 thumb_url = ?8
             WHERE id = ?1",
        )?;
        // New, or with other words: into the search index.
        let mut changed = Vec::new();
        for a in articles {
            let found: Option<(i64, String, String, String)> = exists
                .query_row(params![feed_id, a.guid], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                })
                .optional()?;
            match found {
                Some((id, title, summary, content)) => {
                    if (title, summary, content)
                        != (a.title.clone(), a.summary.clone(), a.content.clone())
                    {
                        changed.push(id);
                    }
                    update.execute(params![
                        id,
                        a.url,
                        a.title,
                        a.author,
                        a.summary,
                        a.content,
                        now,
                        a.thumb_url
                    ])?;
                }
                None => {
                    // Dated in the future (a wrong clock, a scheduled post):
                    // it would sit at the top of the list for good.
                    let published = a.published.map_or(now, |p| p.min(now));
                    let muted = rules.iter().any(|r| r.matches(&a.title, &a.summary));
                    insert.execute(params![
                        feed_id,
                        a.guid,
                        a.url,
                        a.title,
                        a.author,
                        a.summary,
                        a.content,
                        published,
                        now,
                        a.thumb_url,
                        muted
                    ])?;
                    changed.push(tx.last_insert_rowid());
                    added += 1;
                }
            }
        }
        index_articles(&tx, &changed)?;
    }
    tx.commit()?;
    Ok(added)
}

/// Removes read articles, neither starred nor in あとで読む, their feed stopped listing more than
/// `keep_days` ago. 0 keeps everything.
pub fn purge(conn: &Connection, keep_days: u32) -> DbResult<usize> {
    if keep_days == 0 {
        return Ok(0);
    }
    let cutoff = now_ms() - i64::from(keep_days) * 86_400_000;
    conn.execute(
        "DELETE FROM articles WHERE read = 1 AND starred = 0 AND later_at IS NULL AND seen_at < ?1",
        [cutoff],
    )
}

/// Which articles the list shows.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum View {
    All,
    Starred,
    /// あとで読む: oldest first, a queue to work through.
    Later,
    Feed {
        id: i64,
    },
    Folder {
        id: i64,
    },
}

impl View {
    /// Whether 未読のみ applies to this view.
    pub fn filters_unread(self) -> bool {
        !matches!(self, View::Starred | View::Later)
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArticleQuery {
    pub view: View,
    /// Not for スター付き and あとで読む: what is kept there is often already
    /// read, and the list is the filter itself (`View::filters_unread`).
    #[serde(default)]
    pub unread_only: bool,
    #[serde(default)]
    pub search: String,
    /// Listed even when read: the article open in the reader, so reading it
    /// with 未読のみ on doesn't pull it out from under the user.
    #[serde(default)]
    pub keep: Option<i64>,
    /// Oldest first, for the views by date. あとで読む keeps the order it was filled in.
    #[serde(default)]
    pub oldest_first: bool,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ArticleRow {
    pub id: i64,
    pub feed_id: i64,
    pub feed_title: String,
    pub title: String,
    pub summary: String,
    pub url: Option<String>,
    pub published: i64,
    pub read: bool,
    pub starred: bool,
    pub later: bool,
    /// The picture for the 画像付き layout, fetched only for the rows in sight.
    pub thumb_url: Option<String>,
}

/// How much of the summary the list gets: two lines at most are shown.
const LIST_SUMMARY_CHARS: i64 = 160;

/// `WHERE` for a query (without unread / keep) and its parameters.
fn filter(q: &ArticleQuery) -> (String, Vec<rusqlite::types::Value>) {
    use rusqlite::types::Value;
    let mut clauses: Vec<String> = Vec::new();
    let mut args: Vec<Value> = Vec::new();
    match q.view {
        View::All => {}
        View::Starred => clauses.push("a.starred = 1".into()),
        View::Later => clauses.push("a.later_at IS NOT NULL".into()),
        View::Feed { id } => {
            clauses.push("a.feed_id = ?".into());
            args.push(Value::Integer(id));
        }
        View::Folder { id } => {
            clauses.push("f.folder_id = ?".into());
            args.push(Value::Integer(id));
        }
    }
    // docs/SEARCH.md. A word of 3 characters or more is looked up in the
    // index; a shorter one, which a trigram can't find, is looked for in the
    // index's text. The feed's name counts too.
    let groups = crate::search::parse(&q.search);
    if !groups.is_empty() {
        let mut alternatives = Vec::new();
        for group in groups {
            let mut terms = Vec::new();
            for term in group {
                let like = format!("%{}%", escape_like(&term.text));
                let in_text = if term.text.chars().count() >= 3 {
                    args.push(Value::Text(format!(
                        "\"{}\"",
                        term.text.replace('"', "\"\"")
                    )));
                    "a.id IN (SELECT rowid FROM articles_fts WHERE articles_fts MATCH ?)"
                } else {
                    for _ in 0..3 {
                        args.push(Value::Text(like.clone()));
                    }
                    "a.id IN (SELECT rowid FROM articles_fts
                              WHERE title LIKE ? ESCAPE '\\' OR summary LIKE ? ESCAPE '\\' OR body LIKE ? ESCAPE '\\')"
                };
                args.push(Value::Text(like));
                let clause =
                    format!("({in_text} OR COALESCE(f.custom_title, f.title) LIKE ? ESCAPE '\\')");
                terms.push(if term.negated {
                    format!("NOT {clause}")
                } else {
                    clause
                });
            }
            alternatives.push(format!("({})", terms.join(" AND ")));
        }
        clauses.push(format!("({})", alternatives.join(" OR ")));
    }
    let sql = if clauses.is_empty() {
        "1".to_string()
    } else {
        clauses.join(" AND ")
    };
    (sql, args)
}

/// LIKE's wildcards in `s` taken literally (with `ESCAPE '\\'`).
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// The search index (`articles_fts`, docs/SEARCH.md) for these articles,
/// anew: their title, summary and text without the HTML (the page's, once
/// 全文取得 has it).
fn index_articles(conn: &Connection, ids: &[i64]) -> DbResult<()> {
    let mut get = conn.prepare_cached(
        "SELECT title, summary, COALESCE(full_content, content) FROM articles WHERE id = ?1",
    )?;
    let mut delete = conn.prepare_cached("DELETE FROM articles_fts WHERE rowid = ?1")?;
    let mut insert = conn.prepare_cached(
        "INSERT INTO articles_fts (rowid, title, summary, body) VALUES (?1, ?2, ?3, ?4)",
    )?;
    for &id in ids {
        delete.execute([id])?;
        let row = get
            .query_row([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .optional()?;
        if let Some((title, summary, html)) = row {
            insert.execute(params![id, title, summary, crate::feed::plain_text(&html)])?;
        }
    }
    Ok(())
}

/// The articles not in the search index: every one, once, after the upgrade
/// to v8, and those an older app added since. Returns how many.
fn index_missing(conn: &Connection) -> DbResult<usize> {
    let ids: Vec<i64> = {
        let mut stmt = conn
            .prepare("SELECT id FROM articles WHERE id NOT IN (SELECT rowid FROM articles_fts)")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<DbResult<_>>()?
    };
    if !ids.is_empty() {
        let tx = conn.unchecked_transaction()?;
        index_articles(&tx, &ids)?;
        tx.commit()?;
    }
    Ok(ids.len())
}

pub fn query_articles(conn: &Connection, q: &ArticleQuery) -> DbResult<Vec<ArticleRow>> {
    let (mut sql_filter, mut args) = filter(q);
    if q.unread_only && q.view.filters_unread() {
        match q.keep {
            Some(keep) => {
                sql_filter = format!("({sql_filter}) AND (a.read = 0 OR a.id = ?)");
                args.push(rusqlite::types::Value::Integer(keep));
            }
            None => sql_filter = format!("({sql_filter}) AND a.read = 0"),
        }
    }
    let sql = format!(
        "SELECT a.id, a.feed_id, COALESCE(f.custom_title, f.title), a.title, substr(a.summary, 1, {LIST_SUMMARY_CHARS}),
                a.url, a.published, a.read, a.starred, a.later_at IS NOT NULL, a.thumb_url
         FROM articles a JOIN feeds f ON f.id = a.feed_id
         WHERE {sql_filter}
         ORDER BY {order}",
        order = if q.view == View::Later {
            "a.later_at, a.id"
        } else if q.oldest_first {
            "a.published ASC, a.id ASC"
        } else {
            "a.published DESC, a.id DESC"
        }
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(args), |r| {
        Ok(ArticleRow {
            id: r.get(0)?,
            feed_id: r.get(1)?,
            feed_title: r.get(2)?,
            title: r.get(3)?,
            summary: r.get(4)?,
            url: r.get(5)?,
            published: r.get(6)?,
            read: r.get(7)?,
            starred: r.get(8)?,
            later: r.get(9)?,
            thumb_url: r.get(10)?,
        })
    })?;
    rows.collect()
}

/// One article for the reader, with the feed's HTML as stored.
#[derive(Debug)]
pub struct ArticleFull {
    pub id: i64,
    pub feed_id: i64,
    pub feed_title: String,
    pub site_url: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub author: Option<String>,
    pub content: String,
    pub summary: String,
    pub published: i64,
    pub read: bool,
    pub starred: bool,
    pub later: bool,
    /// The feed has 全文取得 on.
    pub full_text: bool,
    pub full_status: Option<FullStatus>,
    pub full_content: Option<String>,
}

pub fn article(conn: &Connection, id: i64) -> DbResult<Option<ArticleFull>> {
    conn.query_row(
        "SELECT a.id, a.feed_id, COALESCE(f.custom_title, f.title), f.site_url, a.title, a.url, a.author,
                a.content, a.summary, a.published, a.read, a.starred, a.later_at IS NOT NULL,
                f.full_text, a.full_status, a.full_content
         FROM articles a JOIN feeds f ON f.id = a.feed_id WHERE a.id = ?1",
        [id],
        |r| {
            Ok(ArticleFull {
                id: r.get(0)?,
                feed_id: r.get(1)?,
                feed_title: r.get(2)?,
                site_url: r.get(3)?,
                title: r.get(4)?,
                url: r.get(5)?,
                author: r.get(6)?,
                content: r.get(7)?,
                summary: r.get(8)?,
                published: r.get(9)?,
                read: r.get(10)?,
                starred: r.get(11)?,
                later: r.get(12)?,
                full_text: r.get(13)?,
                full_status: r.get::<_, Option<i64>>(14)?.and_then(FullStatus::from_db),
                full_content: r.get(15)?,
            })
        },
    )
    .optional()
}

// ------------------------------------------------------------ saved pages

/// The address of the feed that holds saved pages: not a URL anything fetches.
const SAVED_FEED_URL: &str = "feedability:saved";

/// Saves a page from the browser extension into あとで読む (docs/EXTENSION.md):
/// read already (it isn't news), in the queue at the end, or where it was
/// when saved again (the text is updated). Returns the article's id.
pub fn save_page(conn: &mut Connection, page: &NewArticle) -> DbResult<i64> {
    let feed_id = match conn
        .query_row("SELECT id FROM feeds WHERE saved = 1", [], |r| r.get(0))
        .optional()?
    {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO feeds (url, title, added_at, saved) VALUES (?1, '保存したページ', ?2, 1)",
                params![SAVED_FEED_URL, now_ms()],
            )?;
            conn.last_insert_rowid()
        }
    };
    store_articles(conn, feed_id, std::slice::from_ref(page))?;
    let id: i64 = conn.query_row(
        "SELECT id FROM articles WHERE feed_id = ?1 AND guid = ?2",
        params![feed_id, page.guid],
        |r| r.get(0),
    )?;
    conn.execute(
        "UPDATE articles SET read = 1, later_at = COALESCE(later_at, ?2) WHERE id = ?1",
        params![id, now_ms()],
    )?;
    Ok(id)
}

// --------------------------------------------------------------- 全文取得

/// Where an article's text from its page stands (`articles.full_status`).
/// NULL: nothing asked for; it is fetched when the article is opened, if its
/// feed has 全文取得 on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FullStatus {
    /// New in a feed with 全文取得 on: the pass after the refresh fetches it.
    Queued = 0,
    Fetched = 1,
    /// Fetched or extracted without success; not tried again.
    Failed = 2,
}

impl FullStatus {
    fn from_db(v: i64) -> Option<Self> {
        match v {
            0 => Some(Self::Queued),
            1 => Some(Self::Fetched),
            2 => Some(Self::Failed),
            _ => None,
        }
    }
}

/// Turns 全文取得 on or off. Articles already there are not queued (they are
/// fetched when opened); turning it off drops what was still queued. Text
/// already fetched stays, for when it is turned on again.
pub fn set_full_text(conn: &Connection, feed_id: i64, on: bool) -> DbResult<()> {
    conn.execute(
        "UPDATE feeds SET full_text = ?2 WHERE id = ?1",
        params![feed_id, on],
    )?;
    if !on {
        conn.execute(
            "UPDATE articles SET full_status = NULL WHERE feed_id = ?1 AND full_status = 0",
            [feed_id],
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct FullTarget {
    pub id: i64,
    pub url: Option<String>,
}

/// The queued articles to fetch now, newest first, at most `per_feed` of
/// each feed. The rest are taken off the queue: they are fetched when opened
/// (docs/FULL_TEXT.md). The ones returned stay queued until
/// `set_full_content`, so a pass cut short is picked up by the next.
pub fn take_full_queue(conn: &mut Connection, per_feed: usize) -> DbResult<Vec<FullTarget>> {
    let tx = conn.transaction()?;
    let mut targets = Vec::new();
    {
        let mut stmt = tx.prepare(
            "SELECT a.id, a.url, a.feed_id FROM articles a
             WHERE a.full_status = 0 ORDER BY a.feed_id, a.published DESC, a.id DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get::<_, i64>(2)?))
            })?
            .collect::<DbResult<Vec<(i64, Option<String>, i64)>>>()?;
        let mut unqueue = tx.prepare("UPDATE articles SET full_status = NULL WHERE id = ?1")?;
        let (mut feed, mut n) = (None, 0);
        for (id, url, feed_id) in rows {
            if feed != Some(feed_id) {
                (feed, n) = (Some(feed_id), 0);
            }
            n += 1;
            if n <= per_feed {
                targets.push(FullTarget { id, url });
            } else {
                unqueue.execute([id])?;
            }
        }
    }
    tx.commit()?;
    Ok(targets)
}

/// The article's text from its page, or None when it couldn't be had.
///
/// The article opened while the pass after a refresh is on it is fetched by
/// both: a success always stands, and a failure only lands where nothing has
/// come yet, so the one that failed can't undo the one that worked.
pub fn set_full_content(conn: &Connection, id: i64, content: Option<&str>) -> DbResult<()> {
    match content {
        Some(c) => {
            conn.execute(
                "UPDATE articles SET full_content = ?2, full_status = ?3 WHERE id = ?1",
                params![id, c, FullStatus::Fetched as i64],
            )?;
            // The page's text is what the search looks in from now on.
            index_articles(conn, &[id])?;
            1
        }
        None => conn.execute(
            "UPDATE articles SET full_status = ?2
             WHERE id = ?1 AND (full_status IS NULL OR full_status = ?3)",
            params![id, FullStatus::Failed as i64, FullStatus::Queued as i64],
        )?,
    };
    Ok(())
}

pub fn set_read(conn: &mut Connection, ids: &[i64], read: bool) -> DbResult<()> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare("UPDATE articles SET read = ?2 WHERE id = ?1")?;
        for id in ids {
            stmt.execute(params![id, read])?;
        }
    }
    tx.commit()
}

pub fn set_starred(conn: &Connection, id: i64, starred: bool) -> DbResult<()> {
    conn.execute(
        "UPDATE articles SET starred = ?2 WHERE id = ?1",
        params![id, starred],
    )?;
    Ok(())
}

/// Into あとで読む (at the end of the queue) or out of it. An article already
/// in keeps its place.
pub fn set_later(conn: &mut Connection, ids: &[i64], later: bool) -> DbResult<()> {
    let now = now_ms();
    let tx = conn.transaction()?;
    for id in ids {
        if later {
            tx.execute(
                "UPDATE articles SET later_at = COALESCE(later_at, ?2) WHERE id = ?1",
                params![id, now],
            )?;
        } else {
            tx.execute("UPDATE articles SET later_at = NULL WHERE id = ?1", [id])?;
        }
    }
    tx.commit()
}

/// Marks what the query lists as read; returns the ids that were unread, so
/// the toast's 元に戻す can put back exactly those.
pub fn mark_all_read(conn: &mut Connection, q: &ArticleQuery) -> DbResult<Vec<i64>> {
    let (sql_filter, args) = filter(q);
    let ids: Vec<i64> = {
        let sql = format!(
            "SELECT a.id FROM articles a JOIN feeds f ON f.id = a.feed_id WHERE ({sql_filter}) AND a.read = 0"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args), |r| r.get(0))?;
        rows.collect::<DbResult<_>>()?
    };
    set_read(conn, &ids, true)?;
    Ok(ids)
}

// ------------------------------------------------------------------ rules

/// A ミュート rule (docs/MUTE.md).
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: i64,
    /// Words separated by spaces, all of which an article has (as the search).
    pub words: String,
    /// Where it applies: one feed, one folder, or (both None) all of them.
    pub feed_id: Option<i64>,
    pub folder_id: Option<i64>,
    pub enabled: bool,
}

impl Rule {
    /// Whether an article has every word in its title or summary, ignoring
    /// case, as the search does (`filter`).
    pub fn matches(&self, title: &str, summary: &str) -> bool {
        let text = format!("{title}\n{summary}").to_lowercase();
        let mut words = self.words.split_whitespace().peekable();
        words.peek().is_some() && words.all(|w| text.contains(&w.to_lowercase()))
    }
}

const RULE_COLUMNS: &str = "id, words, feed_id, folder_id, enabled";

fn rule_row(r: &rusqlite::Row) -> DbResult<Rule> {
    Ok(Rule {
        id: r.get(0)?,
        words: r.get(1)?,
        feed_id: r.get(2)?,
        folder_id: r.get(3)?,
        enabled: r.get(4)?,
    })
}

pub fn rules(conn: &Connection) -> DbResult<Vec<Rule>> {
    let mut stmt = conn.prepare(&format!("SELECT {RULE_COLUMNS} FROM rules ORDER BY id"))?;
    let rows = stmt.query_map([], rule_row)?;
    rows.collect()
}

/// The rules on, that apply to the feed. None to a saved page's feed: those
/// pages were picked by hand (docs/EXTENSION.md).
fn rules_for_feed(conn: &Connection, feed_id: i64) -> DbResult<Vec<Rule>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {RULE_COLUMNS} FROM rules
         WHERE enabled = 1 AND action = 'read'
           AND EXISTS (SELECT 1 FROM feeds f WHERE f.id = ?1 AND f.saved = 0
                       AND (rules.feed_id = f.id OR rules.folder_id = f.folder_id
                            OR (rules.feed_id IS NULL AND rules.folder_id IS NULL)))"
    ))?;
    let rows = stmt.query_map([feed_id], rule_row)?;
    rows.collect()
}

pub fn add_rule(
    conn: &Connection,
    words: &str,
    feed_id: Option<i64>,
    folder_id: Option<i64>,
) -> DbResult<Rule> {
    let words = words.split_whitespace().collect::<Vec<_>>().join(" ");
    conn.execute(
        "INSERT INTO rules (words, feed_id, folder_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![words, feed_id, folder_id, now_ms()],
    )?;
    conn.query_row(
        &format!("SELECT {RULE_COLUMNS} FROM rules WHERE id = ?1"),
        [conn.last_insert_rowid()],
        rule_row,
    )
}

pub fn set_rule_enabled(conn: &Connection, id: i64, enabled: bool) -> DbResult<()> {
    conn.execute(
        "UPDATE rules SET enabled = ?2 WHERE id = ?1",
        params![id, enabled],
    )?;
    Ok(())
}

pub fn delete_rule(conn: &Connection, id: i64) -> DbResult<()> {
    conn.execute("DELETE FROM rules WHERE id = ?1", [id])?;
    Ok(())
}

/// The rule applied to the unread articles already there, as to new ones.
/// Returns the ids it marked, for 元に戻す.
pub fn apply_rule(conn: &mut Connection, rule: &Rule) -> DbResult<Vec<i64>> {
    let ids: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT a.id, a.title, a.summary FROM articles a JOIN feeds f ON f.id = a.feed_id
             WHERE a.read = 0 AND f.saved = 0
               AND (?1 IS NULL OR a.feed_id = ?1) AND (?2 IS NULL OR f.folder_id = ?2)",
        )?;
        let rows = stmt.query_map(params![rule.feed_id, rule.folder_id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut ids = Vec::new();
        for row in rows {
            let (id, title, summary) = row?;
            if rule.matches(&title, &summary) {
                ids.push(id);
            }
        }
        ids
    };
    set_read(conn, &ids, true)?;
    Ok(ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art(guid: &str, title: &str, published: i64) -> NewArticle {
        NewArticle {
            guid: guid.into(),
            url: Some(format!("https://example.com/{guid}")),
            title: title.into(),
            author: None,
            summary: format!("{title} の要約"),
            content: format!("<p>{title}</p>"),
            published: Some(published),
            thumb_url: None,
        }
    }

    fn all() -> ArticleQuery {
        ArticleQuery {
            view: View::All,
            unread_only: false,
            search: String::new(),
            keep: None,
            oldest_first: false,
        }
    }

    #[test]
    fn stores_new_articles_once_and_keeps_read_state() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        let first = [art("a", "A", 1000), art("b", "B", 2000)];
        assert_eq!(store_articles(&mut conn, feed, &first).unwrap(), 2);

        let rows = query_articles(&conn, &all()).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(),
            ["B", "A"]
        );
        set_read(&mut conn, &[rows[0].id], true).unwrap();

        // The same articles again, one with a corrected title, plus a new one.
        let again = [
            art("a", "A", 1000),
            art("b", "B（訂正）", 2000),
            art("c", "C", 3000),
        ];
        assert_eq!(store_articles(&mut conn, feed, &again).unwrap(), 1);
        let rows = query_articles(&conn, &all()).unwrap();
        let b = rows.iter().find(|r| r.title == "B（訂正）").unwrap();
        assert!(b.read, "an update must not mark it unread again");
        assert_eq!(sidebar(&conn).unwrap().unread, 2);
    }

    #[test]
    fn future_dates_are_clamped() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        store_articles(&mut conn, feed, &[art("x", "X", i64::MAX / 2)]).unwrap();
        let rows = query_articles(&conn, &all()).unwrap();
        assert!(rows[0].published <= now_ms());
    }

    #[test]
    fn views_unread_only_keep_and_search() {
        let mut conn = open_in_memory().unwrap();
        let folder = create_folder(&conn, "技術").unwrap();
        let tech = add_feed(&conn, "https://a.example/feed", "Tech", None, Some(folder)).unwrap();
        let news = add_feed(&conn, "https://b.example/feed", "News", None, None).unwrap();
        store_articles(
            &mut conn,
            tech,
            &[art("1", "Rust 2.0", 1), art("2", "100% safe_code", 2)],
        )
        .unwrap();
        store_articles(&mut conn, news, &[art("3", "天気", 3)]).unwrap();

        let q = |view, unread_only, search: &str, keep| ArticleQuery {
            view,
            unread_only,
            search: search.into(),
            keep,
            oldest_first: false,
        };
        assert_eq!(
            query_articles(&conn, &q(View::Folder { id: folder }, false, "", None))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            query_articles(&conn, &q(View::Feed { id: news }, false, "", None))
                .unwrap()
                .len(),
            1
        );
        // LIKE wildcards in the search are literal.
        assert_eq!(
            query_articles(&conn, &q(View::All, false, "100%", None))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            query_articles(&conn, &q(View::All, false, "e%c", None))
                .unwrap()
                .len(),
            0
        );
        // Every word must match; the feed's name counts.
        assert_eq!(
            query_articles(&conn, &q(View::All, false, "tech rust", None))
                .unwrap()
                .len(),
            1
        );

        let id = query_articles(&conn, &q(View::All, false, "天気", None)).unwrap()[0].id;
        set_read(&mut conn, &[id], true).unwrap();
        assert_eq!(
            query_articles(&conn, &q(View::All, true, "", None))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            query_articles(&conn, &q(View::All, true, "", Some(id)))
                .unwrap()
                .len(),
            3
        );

        set_starred(&conn, id, true).unwrap();
        assert_eq!(
            query_articles(&conn, &q(View::Starred, false, "", None))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(sidebar(&conn).unwrap().starred, 1);
    }

    #[test]
    fn later_is_a_queue_kept_from_purge() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        store_articles(
            &mut conn,
            feed,
            &[art("a", "A", 3), art("b", "B", 2), art("c", "C", 1)],
        )
        .unwrap();
        let id = |t: &str| {
            query_articles(&conn, &all())
                .unwrap()
                .into_iter()
                .find(|r| r.title == t)
                .unwrap()
                .id
        };
        let (a, b, c) = (id("A"), id("B"), id("C"));
        // In the order they went in, not by date; adding again keeps the place.
        set_later(&mut conn, &[b], true).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        set_later(&mut conn, &[a], true).unwrap();
        set_later(&mut conn, &[b], true).unwrap();
        let later = ArticleQuery {
            view: View::Later,
            ..all()
        };
        fn titles(conn: &Connection, q: &ArticleQuery) -> Vec<String> {
            query_articles(conn, q)
                .unwrap()
                .into_iter()
                .map(|r| r.title)
                .collect()
        }
        assert_eq!(titles(&conn, &later), ["B", "A"]);
        assert_eq!(sidebar(&conn).unwrap().later, 2);
        assert!(db_article_later(&conn, a));

        // 未読のみ doesn't hide read articles here.
        set_read(&mut conn, &[a, b, c], true).unwrap();
        let unread_only = ArticleQuery {
            unread_only: true,
            ..later.clone()
        };
        assert_eq!(titles(&conn, &unread_only), ["B", "A"]);

        // Read and old, but in あとで読む: kept.
        conn.execute("UPDATE articles SET seen_at = 0", []).unwrap();
        assert_eq!(purge(&conn, 30).unwrap(), 1, "only C");

        set_later(&mut conn, &[b], false).unwrap();
        assert_eq!(titles(&conn, &later), ["A"]);
    }

    fn db_article_later(conn: &Connection, id: i64) -> bool {
        article(conn, id).unwrap().unwrap().later
    }

    #[test]
    fn oldest_first_reverses_the_order_but_not_in_later() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        store_articles(
            &mut conn,
            feed,
            &[
                art("a", "A", 1000),
                art("b", "B", 2000),
                art("c", "C", 3000),
            ],
        )
        .unwrap();
        fn titles(conn: &Connection, q: &ArticleQuery) -> Vec<String> {
            query_articles(conn, q)
                .unwrap()
                .into_iter()
                .map(|r| r.title)
                .collect()
        }
        let feed_q = ArticleQuery {
            view: View::Feed { id: feed },
            ..all()
        };
        assert_eq!(titles(&conn, &feed_q), ["C", "B", "A"]);
        let oldest = ArticleQuery {
            oldest_first: true,
            ..feed_q
        };
        assert_eq!(titles(&conn, &oldest), ["A", "B", "C"]);

        // あとで読む stays in the order they went in.
        let id = |t: &str| {
            query_articles(&conn, &all())
                .unwrap()
                .into_iter()
                .find(|r| r.title == t)
                .unwrap()
                .id
        };
        let (a, c) = (id("A"), id("C"));
        set_later(&mut conn, &[c], true).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        set_later(&mut conn, &[a], true).unwrap();
        for oldest_first in [false, true] {
            let later = ArticleQuery {
                view: View::Later,
                oldest_first,
                ..all()
            };
            assert_eq!(titles(&conn, &later), ["C", "A"]);
        }
    }

    #[test]
    fn migrates_a_version_1_file() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE feeds (id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, title TEXT NOT NULL,
               custom_title TEXT, site_url TEXT, folder_id INTEGER, added_at INTEGER NOT NULL,
               fetched_at INTEGER, error TEXT, etag TEXT, last_modified TEXT);
             CREATE TABLE articles (id INTEGER PRIMARY KEY, feed_id INTEGER NOT NULL, guid TEXT NOT NULL,
               url TEXT, title TEXT NOT NULL, author TEXT, summary TEXT NOT NULL DEFAULT '',
               content TEXT NOT NULL DEFAULT '', published INTEGER NOT NULL, added_at INTEGER NOT NULL,
               seen_at INTEGER NOT NULL, read INTEGER NOT NULL DEFAULT 0, starred INTEGER NOT NULL DEFAULT 0,
               UNIQUE (feed_id, guid));
             PRAGMA user_version = 1;",
        )
        .unwrap();
        setup(&conn).unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, VERSION);
        conn.execute("UPDATE articles SET later_at = 1", [])
            .unwrap();
        conn.execute("UPDATE feeds SET icon = 'x', icon_checked_at = 1", [])
            .unwrap();
        conn.execute("UPDATE articles SET thumb_url = 'x'", [])
            .unwrap();
    }

    #[test]
    fn keeps_the_picture_up_to_date() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        let with = |thumb: Option<&str>| NewArticle {
            thumb_url: thumb.map(String::from),
            ..art("a", "A", 1000)
        };
        let thumb = |conn: &Connection| query_articles(conn, &all()).unwrap()[0].thumb_url.clone();
        store_articles(&mut conn, feed, &[with(None)]).unwrap();
        assert_eq!(thumb(&conn), None);
        // An article stored before the column gets its picture on the next refresh.
        store_articles(&mut conn, feed, &[with(Some("https://example.com/a.jpg"))]).unwrap();
        assert_eq!(thumb(&conn).as_deref(), Some("https://example.com/a.jpg"));
    }

    #[test]
    fn migrates_a_version_2_file() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE feeds (id INTEGER PRIMARY KEY, url TEXT NOT NULL UNIQUE, title TEXT NOT NULL,
               custom_title TEXT, site_url TEXT, folder_id INTEGER, added_at INTEGER NOT NULL,
               fetched_at INTEGER, error TEXT, etag TEXT, last_modified TEXT);
             CREATE TABLE articles (id INTEGER PRIMARY KEY);
             INSERT INTO feeds (id, url, title, added_at, etag, last_modified)
               VALUES (1, 'https://a.example/feed', 'A', 0, 'v1', 'Sat, 17 Oct 2026 00:00:00 GMT');
             PRAGMA user_version = 2;",
        )
        .unwrap();
        setup(&conn).unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, VERSION);
        // The existing feed has never been looked up.
        assert_eq!(feeds_needing_icon(&conn, 0).unwrap().len(), 1);
        // Version 4: read in full once, so its articles get their pictures.
        let t = &feed_targets(&conn, None).unwrap()[0];
        assert_eq!(
            (t.etag.as_deref(), t.last_modified.as_deref()),
            (None, None)
        );
        set_icon(
            &conn,
            1,
            "https://a.example/feed",
            Some("data:image/png;base64,AA=="),
        )
        .unwrap();
        assert_eq!(icons(&conn).unwrap().len(), 1);
    }

    #[test]
    fn mark_all_read_returns_what_it_changed() {
        let mut conn = open_in_memory().unwrap();
        let a = add_feed(&conn, "https://a.example/feed", "A", None, None).unwrap();
        let b = add_feed(&conn, "https://b.example/feed", "B", None, None).unwrap();
        store_articles(&mut conn, a, &[art("1", "one", 1), art("2", "two", 2)]).unwrap();
        store_articles(&mut conn, b, &[art("3", "three", 3)]).unwrap();
        let one = query_articles(&conn, &all())
            .unwrap()
            .into_iter()
            .find(|r| r.title == "one")
            .unwrap();
        set_read(&mut conn, &[one.id], true).unwrap();

        let q = ArticleQuery {
            view: View::Feed { id: a },
            ..all()
        };
        let changed = mark_all_read(&mut conn, &q).unwrap();
        assert_eq!(changed.len(), 1, "only the unread one");
        let s = sidebar(&conn).unwrap();
        assert_eq!(s.unread, 1);
        assert_eq!(s.feeds.iter().find(|f| f.id == b).unwrap().unread, 1);

        set_read(&mut conn, &changed, false).unwrap();
        assert_eq!(sidebar(&conn).unwrap().unread, 2);
    }

    #[test]
    fn purge_keeps_what_the_feed_still_lists() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        store_articles(
            &mut conn,
            feed,
            &[
                art("old", "old", 1),
                art("starred", "starred", 2),
                art("unread", "unread", 3),
            ],
        )
        .unwrap();
        let rows = query_articles(&conn, &all()).unwrap();
        let id = |t: &str| rows.iter().find(|r| r.title == t).unwrap().id;
        set_read(&mut conn, &[id("old"), id("starred")], true).unwrap();
        set_starred(&conn, id("starred"), true).unwrap();

        // Seen long ago: the feed no longer lists them.
        conn.execute("UPDATE articles SET seen_at = 0", []).unwrap();
        assert_eq!(purge(&conn, 0).unwrap(), 0, "0 keeps everything");
        assert_eq!(purge(&conn, 30).unwrap(), 1);
        let left: Vec<String> = query_articles(&conn, &all())
            .unwrap()
            .into_iter()
            .map(|r| r.title)
            .collect();
        assert_eq!(left, ["unread", "starred"]);

        // A 304 marks everything as seen now.
        set_read(&mut conn, &[id("unread")], true).unwrap();
        feed_unchanged(&conn, feed).unwrap();
        assert_eq!(purge(&conn, 30).unwrap(), 0);
    }

    #[test]
    fn folders_and_feeds() {
        let mut conn = open_in_memory().unwrap();
        let folder = folder_named(&conn, "ニュース").unwrap();
        assert_eq!(folder_named(&conn, "ニュース").unwrap(), folder);
        let feed = add_feed(
            &conn,
            "https://example.com/feed",
            "Example",
            None,
            Some(folder),
        )
        .unwrap();
        store_articles(&mut conn, feed, &[art("1", "one", 1)]).unwrap();
        assert_eq!(sidebar(&conn).unwrap().folders[0].unread, 1);

        rename_feed(&conn, feed, "  私の名前 ").unwrap();
        feed_fetched(
            &conn,
            feed,
            Some("New feed title"),
            None,
            Some("\"e1\""),
            None,
        )
        .unwrap();
        assert_eq!(sidebar(&conn).unwrap().feeds[0].title, "私の名前");
        rename_feed(&conn, feed, "").unwrap();
        assert_eq!(sidebar(&conn).unwrap().feeds[0].title, "New feed title");
        assert_eq!(
            feed_targets(&conn, None).unwrap()[0].etag.as_deref(),
            Some("\"e1\"")
        );

        delete_folder(&conn, folder).unwrap();
        assert_eq!(sidebar(&conn).unwrap().feeds[0].folder_id, None);

        let export = feeds_for_export(&conn).unwrap();
        assert_eq!(export.len(), 1);
        delete_feed(&conn, feed).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM articles", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "articles go with their feed");
    }

    #[test]
    fn feed_info_counts_and_names() {
        let mut conn = open_in_memory().unwrap();
        assert!(feed_info(&conn, 1).unwrap().is_none());
        let feed = add_feed(
            &conn,
            "https://example.com/feed",
            "Example",
            Some("https://example.com/"),
            None,
        )
        .unwrap();
        store_articles(&mut conn, feed, &[art("1", "one", 1), art("2", "two", 2)]).unwrap();
        set_read(&mut conn, &[1], true).unwrap();
        feed_failed(&conn, feed, "見つかりません（HTTP 404）").unwrap();
        rename_feed(&conn, feed, "私の名前").unwrap();
        let info = feed_info(&conn, feed).unwrap().unwrap();
        assert_eq!(info.title, "私の名前");
        assert_eq!(info.original_title, "Example");
        assert_eq!(info.custom_title.as_deref(), Some("私の名前"));
        assert_eq!(info.url, "https://example.com/feed");
        assert_eq!(info.site_url.as_deref(), Some("https://example.com/"));
        assert_eq!(info.error.as_deref(), Some("見つかりません（HTTP 404）"));
        assert_eq!((info.articles, info.unread), (2, 1));
        assert!(info.fetched_at.is_some());
        assert!(info.added_at > 0);
        rename_feed(&conn, feed, "").unwrap();
        assert_eq!(feed_info(&conn, feed).unwrap().unwrap().title, "Example");
    }

    #[test]
    fn changing_the_url_clears_what_belonged_to_the_old_one() {
        let mut conn = open_in_memory().unwrap();
        let a = add_feed(
            &conn,
            "https://a.example/feed",
            "A",
            Some("https://a.example/"),
            None,
        )
        .unwrap();
        let b = add_feed(&conn, "https://b.example/feed", "B", None, None).unwrap();
        store_articles(&mut conn, a, &[art("1", "one", 1)]).unwrap();
        rename_feed(&conn, a, "私の名前").unwrap();
        feed_fetched(
            &conn,
            a,
            None,
            None,
            Some("\"e1\""),
            Some("Mon, 01 Jan 2024"),
        )
        .unwrap();
        set_icon_urls(&conn, a, Some("https://a.example/icon.png"), None).unwrap();
        set_icon(
            &conn,
            a,
            "https://a.example/feed",
            Some("data:image/png;base64,AA=="),
        )
        .unwrap();
        feed_failed(&conn, a, "見つかりません（HTTP 404）").unwrap();

        // Another feed's URL: refused, and nothing changed.
        assert!(change_feed_url(&conn, a, "https://b.example/feed").is_err());
        assert_eq!(
            feed_info(&conn, a).unwrap().unwrap().url,
            "https://a.example/feed"
        );

        assert!(change_feed_url(&conn, a, "https://a.example/new.xml").unwrap());
        let info = feed_info(&conn, a).unwrap().unwrap();
        assert_eq!(info.url, "https://a.example/new.xml");
        assert_eq!(info.title, "私の名前", "the user's name stays");
        assert_eq!(info.original_title, "A");
        assert_eq!(info.error, None);
        assert_eq!(info.site_url, None, "the old site goes with the old URL");
        assert_eq!(info.articles, 1, "articles stay");
        let t = feed_targets(&conn, Some(&[a][..])).unwrap().remove(0);
        assert_eq!((t.etag, t.last_modified), (None, None));
        // The icon is looked up again.
        assert!(icons(&conn).unwrap().is_empty());
        let needing = feeds_needing_icon(&conn, 0).unwrap();
        assert!(needing.iter().any(|t| t.id == a && t.icon_url.is_none()));
        // A lookup for the old URL finishing late writes nothing, and the new
        // site's icon is still to be looked up.
        set_icon(
            &conn,
            a,
            "https://a.example/feed",
            Some("data:image/png;base64,AA=="),
        )
        .unwrap();
        assert!(icons(&conn).unwrap().is_empty());
        let needing = feeds_needing_icon(&conn, now_ms() - 86_400_000).unwrap();
        assert!(needing.iter().any(|t| t.id == a));

        // The same feed again is its own URL, not a conflict with itself.
        assert!(change_feed_url(&conn, a, "https://a.example/new.xml").unwrap());
        assert!(!change_feed_url(&conn, 999, "https://c.example/feed").unwrap());
        assert!(feed_info(&conn, b).unwrap().is_some());
    }

    #[test]
    fn icons_are_looked_up_once_in_a_while() {
        let conn = open_in_memory().unwrap();
        let a = add_feed(&conn, "https://a.example/feed", "A", None, None).unwrap();
        let b = add_feed(&conn, "https://b.example/feed", "B", None, None).unwrap();
        set_icon_urls(
            &conn,
            a,
            Some("https://a.example/i.png"),
            Some("https://a.example/l.png"),
        )
        .unwrap();
        let day = 86_400_000;
        let cutoff = now_ms() - 30 * day;
        let todo = feeds_needing_icon(&conn, cutoff).unwrap();
        assert_eq!(todo.iter().map(|t| t.id).collect::<Vec<_>>(), [a, b]);
        assert_eq!(todo[0].icon_url.as_deref(), Some("https://a.example/i.png"));
        assert_eq!(todo[0].logo_url.as_deref(), Some("https://a.example/l.png"));

        set_icon(
            &conn,
            a,
            "https://a.example/feed",
            Some("data:image/png;base64,AA=="),
        )
        .unwrap();
        // Nothing found still counts as a check, and keeps what was there.
        set_icon(&conn, b, "https://b.example/feed", None).unwrap();
        assert!(feeds_needing_icon(&conn, cutoff).unwrap().is_empty());
        assert_eq!(
            icons(&conn).unwrap(),
            [(a, "data:image/png;base64,AA==".to_string())]
        );
        set_icon(&conn, a, "https://a.example/feed", None).unwrap();
        assert_eq!(icons(&conn).unwrap().len(), 1);
        // Thirty days on, both are due again.
        let later = now_ms() + day;
        assert_eq!(feeds_needing_icon(&conn, later).unwrap().len(), 2);
    }

    #[test]
    fn reopening_keeps_the_data() {
        let dir = std::env::temp_dir().join(format!("feedability-test-{}", now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("feedability.db");
        let conn = open(&path).unwrap();
        add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        close(conn);
        let conn = open(&path).unwrap();
        assert_eq!(sidebar(&conn).unwrap().feeds.len(), 1);
        close(conn);
        assert!(
            !dir.join("feedability.db-wal").exists()
                || std::fs::metadata(dir.join("feedability.db-wal"))
                    .unwrap()
                    .len()
                    == 0
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn full_text_queues_new_articles_only() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        store_articles(&mut conn, feed, &[art("old", "Old", 1)]).unwrap();
        let id = |c: &Connection, t: &str| -> i64 {
            c.query_row("SELECT id FROM articles WHERE title = ?1", [t], |r| {
                r.get(0)
            })
            .unwrap()
        };
        let old = id(&conn, "Old");

        // Turned on: what was there waits to be opened; new ones are queued.
        set_full_text(&conn, feed, true).unwrap();
        assert!(sidebar(&conn).unwrap().feeds[0].full_text);
        assert!(feed_info(&conn, feed).unwrap().unwrap().full_text);
        let news: Vec<NewArticle> = (0..25)
            .map(|i| art(&format!("n{i}"), &format!("N{i}"), 10 + i))
            .collect();
        store_articles(&mut conn, feed, &news).unwrap();
        assert_eq!(article(&conn, old).unwrap().unwrap().full_status, None);

        // 20 of them, newest first; the other 5 are left for when they are opened.
        let queue = take_full_queue(&mut conn, 20).unwrap();
        assert_eq!(queue.len(), 20);
        assert_eq!(queue[0].id, id(&conn, "N24"));
        assert_eq!(queue[0].url.as_deref(), Some("https://example.com/n24"));
        // Taken, they stay queued until fetched (a pass cut short by quitting
        // the app is picked up by the next one).
        assert_eq!(take_full_queue(&mut conn, 20).unwrap().len(), 20);
        assert_eq!(
            article(&conn, id(&conn, "N0"))
                .unwrap()
                .unwrap()
                .full_status,
            None
        );
        assert_eq!(
            article(&conn, queue[0].id).unwrap().unwrap().full_status,
            Some(FullStatus::Queued)
        );

        set_full_content(&conn, queue[0].id, Some("<p>全文</p>")).unwrap();
        // The same article fetched again at the same time and failing this
        // time: what worked stays.
        set_full_content(&conn, queue[0].id, None).unwrap();
        set_full_content(&conn, queue[1].id, None).unwrap();
        let a = article(&conn, queue[0].id).unwrap().unwrap();
        assert_eq!(a.full_status, Some(FullStatus::Fetched));
        assert_eq!(a.full_content.as_deref(), Some("<p>全文</p>"));
        assert!(a.full_text);
        assert_eq!(
            article(&conn, queue[1].id).unwrap().unwrap().full_status,
            Some(FullStatus::Failed)
        );

        // The feed listing the article again keeps the text from the page.
        store_articles(&mut conn, feed, &news).unwrap();
        let a = article(&conn, queue[0].id).unwrap().unwrap();
        assert_eq!(a.full_content.as_deref(), Some("<p>全文</p>"));

        // Turned off: nothing new is queued, the queue is dropped, the text stays.
        store_articles(&mut conn, feed, &[art("q", "Q", 50)]).unwrap();
        set_full_text(&conn, feed, false).unwrap();
        store_articles(&mut conn, feed, &[art("r", "R", 60)]).unwrap();
        assert!(take_full_queue(&mut conn, 20).unwrap().is_empty());
        assert_eq!(
            article(&conn, id(&conn, "Q")).unwrap().unwrap().full_status,
            None
        );
        let a = article(&conn, queue[0].id).unwrap().unwrap();
        assert!(!a.full_text);
        assert_eq!(a.full_content.as_deref(), Some("<p>全文</p>"));
    }

    #[test]
    fn a_version_4_file_gets_the_full_text_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 0).unwrap();
        migrate(&conn).unwrap();
        // Run again as if opened by this version once more: nothing to do.
        migrate(&conn).unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, VERSION);
        conn.prepare("SELECT full_text FROM feeds").unwrap();
        conn.prepare("SELECT full_content, full_status FROM articles")
            .unwrap();
    }

    #[test]
    fn saved_pages_go_to_later_and_stay_out_of_the_feeds() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://example.com/feed", "Example", None, None).unwrap();
        let page = crate::feed::page_article(
            "https://news.example.com/a",
            "保存した記事",
            "<p>本文</p>",
            1_000,
        );
        let id = save_page(&mut conn, &page).unwrap();
        let a = article(&conn, id).unwrap().unwrap();
        assert!(a.later && a.read);
        assert_eq!(a.feed_title, "保存したページ");
        assert_eq!(sidebar(&conn).unwrap().later, 1);

        // Not a feed anywhere: the sidebar, a refresh, the icons, OPML.
        let s = sidebar(&conn).unwrap();
        assert_eq!(s.feeds.len(), 1);
        assert_eq!(s.feeds[0].id, feed);
        assert_eq!(feed_targets(&conn, None).unwrap().len(), 1);
        assert_eq!(feeds_needing_icon(&conn, 0).unwrap().len(), 1);
        assert_eq!(feeds_for_export(&conn).unwrap().len(), 1);

        // Saved again: the same article, its place kept, the text updated.
        set_later(&mut conn, &[id], true).unwrap();
        let again = crate::feed::page_article(
            "https://news.example.com/a",
            "保存した記事",
            "<p>新しい本文</p>",
            2_000,
        );
        assert_eq!(save_page(&mut conn, &again).unwrap(), id);
        assert_eq!(
            article(&conn, id).unwrap().unwrap().content,
            "<p>新しい本文</p>"
        );
        assert_eq!(sidebar(&conn).unwrap().later, 1);
    }

    fn read_of(conn: &Connection, guid: &str) -> (bool, Option<i64>) {
        conn.query_row(
            "SELECT read, full_status FROM articles WHERE guid = ?1",
            [guid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn a_rule_marks_new_articles_read() {
        let mut conn = open_in_memory().unwrap();
        let folder = create_folder(&conn, "技術").unwrap();
        let tech = add_feed(&conn, "https://a.example/feed", "A", None, Some(folder)).unwrap();
        let news = add_feed(&conn, "https://b.example/feed", "B", None, None).unwrap();
        set_full_text(&conn, tech, true).unwrap();
        add_rule(&conn, "  セール\tamazon ", None, Some(folder)).unwrap();
        let off = add_rule(&conn, "日記", None, None).unwrap();
        set_rule_enabled(&conn, off.id, false).unwrap();
        assert_eq!(rules(&conn).unwrap()[0].words, "セール amazon");

        let added = store_articles(
            &mut conn,
            tech,
            &[
                art("t1", "Amazon のセール", 1),
                art("t2", "セールの話", 2),
                art("t3", "日記", 3),
            ],
        )
        .unwrap();
        assert_eq!(added, 3);
        // Every word, any case; not queued for 全文取得.
        assert_eq!(read_of(&conn, "t1"), (true, None));
        assert_eq!(read_of(&conn, "t2"), (false, Some(0)));
        // A rule turned off does nothing.
        assert_eq!(read_of(&conn, "t3"), (false, Some(0)));
        // Outside the folder.
        store_articles(&mut conn, news, &[art("n1", "AMAZON セール", 1)]).unwrap();
        assert_eq!(read_of(&conn, "n1"), (false, None));

        // Marked unread by hand, it stays so on the next refresh.
        let t1 = article_id(&conn, "t1");
        set_read(&mut conn, &[t1], false).unwrap();
        store_articles(&mut conn, tech, &[art("t1", "Amazon のセール", 1)]).unwrap();
        assert!(!read_of(&conn, "t1").0);
    }

    fn article_id(conn: &Connection, guid: &str) -> i64 {
        conn.query_row("SELECT id FROM articles WHERE guid = ?1", [guid], |r| {
            r.get(0)
        })
        .unwrap()
    }

    #[test]
    fn a_new_rule_applies_to_the_unread_articles_there() {
        let mut conn = open_in_memory().unwrap();
        let a = add_feed(&conn, "https://a.example/feed", "A", None, None).unwrap();
        let b = add_feed(&conn, "https://b.example/feed", "B", None, None).unwrap();
        store_articles(
            &mut conn,
            a,
            &[art("a1", "PR 記事", 1), art("a2", "本文", 2)],
        )
        .unwrap();
        store_articles(&mut conn, b, &[art("b1", "pr 記事", 1)]).unwrap();
        save_page(&mut conn, &art("https://saved.example/", "PR 記事", 3)).unwrap();
        let in_a = add_rule(&conn, "pr", Some(a), None).unwrap();
        assert_eq!(
            apply_rule(&mut conn, &in_a).unwrap(),
            [article_id(&conn, "a1")]
        );
        assert!(!read_of(&conn, "b1").0);

        // All feeds, but not a page saved by hand; nothing twice.
        let everywhere = add_rule(&conn, "PR", None, None).unwrap();
        assert_eq!(
            apply_rule(&mut conn, &everywhere).unwrap(),
            [article_id(&conn, "b1")]
        );
    }

    #[test]
    fn rules_go_with_their_feed_or_folder() {
        let conn = open_in_memory().unwrap();
        let folder = create_folder(&conn, "F").unwrap();
        let feed = add_feed(&conn, "https://a.example/feed", "A", None, None).unwrap();
        add_rule(&conn, "x", Some(feed), None).unwrap();
        add_rule(&conn, "y", None, Some(folder)).unwrap();
        let all = add_rule(&conn, "z", None, None).unwrap();
        delete_feed(&conn, feed).unwrap();
        delete_folder(&conn, folder).unwrap();
        assert_eq!(rules(&conn).unwrap(), std::slice::from_ref(&all));
        delete_rule(&conn, all.id).unwrap();
        assert!(rules(&conn).unwrap().is_empty());
    }

    #[test]
    fn an_empty_rule_matches_nothing() {
        let rule = Rule {
            id: 1,
            words: " ".into(),
            feed_id: None,
            folder_id: None,
            enabled: true,
        };
        assert!(!rule.matches("何でも", ""));
    }

    #[test]
    fn the_search_looks_in_the_text() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://a.example/feed", "Blog", None, None).unwrap();
        let with = |guid: &str, title: &str, body: &str| NewArticle {
            content: body.into(),
            summary: String::new(),
            ..art(guid, title, 1)
        };
        store_articles(
            &mut conn,
            feed,
            &[
                with(
                    "1",
                    "お知らせ",
                    "<p>新しい<b>バージョン</b>を公開しました。Tauri 2 対応</p>",
                ),
                with("2", "日記", "<p>今日は東京で雨。</p>"),
                with("3", "Release notes", "<p>Bug fixes &amp; tauri updates</p>"),
            ],
        )
        .unwrap();
        let found = |conn: &Connection, search: &str| -> Vec<String> {
            let q = ArticleQuery {
                search: search.into(),
                ..all()
            };
            let mut t: Vec<String> = query_articles(conn, &q)
                .unwrap()
                .into_iter()
                .map(|r| r.title)
                .collect();
            t.sort();
            t
        };
        // Inside a run of Japanese, across the tags, any case.
        assert_eq!(found(&conn, "バージョン"), ["お知らせ"]);
        assert_eq!(found(&conn, "TAURI"), ["Release notes", "お知らせ"]);
        // Shorter than a trigram.
        assert_eq!(found(&conn, "東京"), ["日記"]);
        assert_eq!(found(&conn, "雨"), ["日記"]);
        // A phrase, a word left out, either of two.
        assert_eq!(found(&conn, r#""tauri 2""#), ["お知らせ"]);
        assert_eq!(found(&conn, "tauri -公開"), ["Release notes"]);
        assert_eq!(found(&conn, "東京 OR fixes"), ["Release notes", "日記"]);
        // The feed's name still counts; quotes in a word are only text.
        assert_eq!(found(&conn, "blog 雨").len(), 1);
        assert!(found(&conn, r#"a"b"c"#).is_empty());

        // The page's text once 全文取得 has it; a changed article anew.
        let id = query_articles(
            &conn,
            &ArticleQuery {
                search: "日記".into(),
                ..all()
            },
        )
        .unwrap()[0]
            .id;
        set_full_content(&conn, id, Some("<p>晴れのち曇り</p>")).unwrap();
        assert_eq!(found(&conn, "晴れのち"), ["日記"]);
        assert!(found(&conn, "東京").is_empty());
        store_articles(
            &mut conn,
            feed,
            &[with("3", "Release notes", "<p>Only docs</p>")],
        )
        .unwrap();
        assert!(found(&conn, "fixes").is_empty());

        // Gone from the index with the article.
        delete_feed(&conn, feed).unwrap();
        let left: i64 = conn
            .query_row("SELECT COUNT(*) FROM articles_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn articles_missing_from_the_index_are_added() {
        let mut conn = open_in_memory().unwrap();
        let feed = add_feed(&conn, "https://a.example/feed", "A", None, None).unwrap();
        store_articles(&mut conn, feed, &[art("1", "検索できる記事", 1)]).unwrap();
        // As after the upgrade to v8, or an older app's refresh.
        conn.execute("DELETE FROM articles_fts", []).unwrap();
        assert_eq!(index_missing(&conn).unwrap(), 1);
        assert_eq!(index_missing(&conn).unwrap(), 0);
        let q = ArticleQuery {
            search: "できる".into(),
            ..all()
        };
        assert_eq!(query_articles(&conn, &q).unwrap().len(), 1);
    }
}
