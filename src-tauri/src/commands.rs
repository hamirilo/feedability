//! Commands invoked from the frontend (src/lib/api.ts). All return
//! `Result<_, String>` with a message the user can read, so errors surface
//! as rejected promises in JS.
//!
//! They are `async`: Tauri runs synchronous commands on the main thread, and
//! even a short database query there stalls the window. The database is one
//! connection behind a mutex, held only for the query itself, never across a
//! network request.

use crate::db::{self, ArticleQuery, ArticleRow, Sidebar};
use crate::fetch::{self, Outcome};
use crate::{feed, opml, sanitize};
use futures_util::StreamExt;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

pub type CmdResult<T> = Result<T, String>;

#[derive(Default)]
pub struct AppState {
    pub db: Mutex<Option<Connection>>,
    /// A refresh is running (one at a time: the timer and ⌘R can meet).
    refreshing: AtomicBool,
    /// Asked to stop the running refresh (the progress toast's 中止).
    cancel: AtomicBool,
    /// An icon pass is running, and another was asked for meanwhile (a feed
    /// was added): one pass at a time, run again if asked.
    icons_running: AtomicBool,
    icons_again: AtomicBool,
}

pub(crate) fn err(e: impl ToString) -> String {
    e.to_string()
}

fn with_db<T>(
    state: &AppState,
    f: impl FnOnce(&mut Connection) -> db::DbResult<T>,
) -> CmdResult<T> {
    let mut guard = state.db.lock().unwrap_or_else(|e| e.into_inner());
    let conn = guard
        .as_mut()
        .ok_or("データを開けませんでした。ログを確認してください")?;
    f(conn).map_err(err)
}

/// feedability.db in the app's data folder, opened at launch (lib.rs).
pub fn open_db(app: &AppHandle) -> CmdResult<()> {
    let dir = app.path().app_data_dir().map_err(err)?;
    fs::create_dir_all(&dir).map_err(err)?;
    let path = dir.join("feedability.db");
    let conn = db::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    log::info!("data: {}", path.display());
    *app.state::<AppState>().db.lock().unwrap() = Some(conn);
    Ok(())
}

/// Written back and closed on quit (lib.rs), so the file is whole on its own.
pub fn close_db(app: &AppHandle) {
    if let Some(conn) = app
        .state::<AppState>()
        .db
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
    {
        db::close(conn);
    }
}

// ------------------------------------------------------------- settings

/// app_config_dir()/settings.json: the settings dialog (docs/SETTINGS.md).
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Look for a new version a few seconds after launch.
    auto_update: bool,
    /// Refresh every feed this often while the app runs; 0 = only by hand.
    refresh_minutes: u32,
    /// Read articles their feed no longer lists go after this many days; 0 = never.
    keep_days: u32,
    /// 外観: the OS's, or always light / dark (docs/THEME.md).
    pub theme: Theme,
    /// After すべて既読, move on to the next feed with unread articles.
    next_feed_after_read_all: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// The window's page colour (index.css `--color-bg`) for each theme: what
/// shows before the page has drawn, so opening doesn't flash the other one.
const BG_DARK: tauri::window::Color = tauri::window::Color(0x1b, 0x1c, 0x1f, 0xff);
const BG_LIGHT: tauri::window::Color = tauri::window::Color(0xf7, 0xf7, 0xf8, 0xff);

/// The window follows the theme: its title bar, scroll bars and native
/// controls, and `prefers-color-scheme` inside the page. Done from Rust so
/// the page needs no window permission for it.
pub fn apply_theme(window: &tauri::WebviewWindow, theme: Theme) {
    let wanted = match theme {
        Theme::System => None,
        Theme::Light => Some(tauri::Theme::Light),
        Theme::Dark => Some(tauri::Theme::Dark),
    };
    if let Err(e) = window.set_theme(wanted) {
        log::warn!("theme: {e}");
    }
    let dark = window.theme().map_or(true, |t| t == tauri::Theme::Dark);
    let _ = window.set_background_color(Some(if dark { BG_DARK } else { BG_LIGHT }));
}

/// The saved theme, for the window at launch (lib.rs).
pub fn saved_theme(app: &AppHandle) -> Theme {
    load_settings(app).theme
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            auto_update: true,
            refresh_minutes: 30,
            keep_days: 30,
            theme: Theme::System,
            next_feed_after_read_all: false,
        }
    }
}

fn settings_path(app: &AppHandle) -> CmdResult<PathBuf> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(err)?
        .join("settings.json"))
}

fn load_settings(app: &AppHandle) -> AppSettings {
    let Ok(path) = settings_path(app) else {
        return AppSettings::default();
    };
    let Ok(bytes) = fs::read(&path) else {
        return AppSettings::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        // Kept aside rather than overwritten by the next save.
        log::warn!("{}: 読み込めないため退避します: {e}", path.display());
        let _ = fs::rename(
            &path,
            path.with_extension(format!("json.bad-{}", db::now_ms())),
        );
        AppSettings::default()
    })
}

#[tauri::command]
pub async fn get_app_settings(app: AppHandle) -> AppSettings {
    load_settings(&app)
}

/// Written to a temporary file and renamed over the old one, so a crash
/// mid-write never leaves half a file.
#[tauri::command]
pub async fn set_app_settings(app: AppHandle, settings: AppSettings) -> CmdResult<AppSettings> {
    if let Some(w) = app.get_webview_window("main") {
        apply_theme(&w, settings.theme);
    }
    let path = settings_path(&app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&settings).map_err(err)?).map_err(err)?;
    fs::rename(&tmp, &path).map_err(err)?;
    Ok(settings)
}

// -------------------------------------------------------------- reading

#[tauri::command]
pub async fn get_sidebar(state: State<'_, AppState>) -> CmdResult<Sidebar> {
    with_db(&state, |c| db::sidebar(c))
}

#[tauri::command]
pub async fn query_articles(
    state: State<'_, AppState>,
    query: ArticleQuery,
) -> CmdResult<Vec<ArticleRow>> {
    with_db(&state, |c| db::query_articles(c, &query))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Article {
    id: i64,
    feed_id: i64,
    feed_title: String,
    site_url: Option<String>,
    title: String,
    url: Option<String>,
    author: Option<String>,
    /// Sanitized (sanitize.rs); images still point at their servers.
    html: String,
    published: i64,
    read: bool,
    starred: bool,
    later: bool,
}

#[tauri::command]
pub async fn get_article(state: State<'_, AppState>, id: i64) -> CmdResult<Article> {
    let a = with_db(&state, |c| db::article(c, id))?.ok_or("記事が見つかりません")?;
    let base = a.url.as_deref().or(a.site_url.as_deref());
    let source = if a.content.trim().is_empty() {
        &a.summary
    } else {
        &a.content
    };
    Ok(Article {
        html: sanitize::article_html(source, base),
        id: a.id,
        feed_id: a.feed_id,
        feed_title: a.feed_title,
        site_url: a.site_url,
        title: a.title,
        url: a.url,
        author: a.author,
        published: a.published,
        read: a.read,
        starred: a.starred,
        later: a.later,
    })
}

#[tauri::command]
pub async fn set_read(state: State<'_, AppState>, ids: Vec<i64>, read: bool) -> CmdResult<()> {
    with_db(&state, |c| db::set_read(c, &ids, read))
}

#[tauri::command]
pub async fn set_starred(state: State<'_, AppState>, id: i64, starred: bool) -> CmdResult<()> {
    with_db(&state, |c| db::set_starred(c, id, starred))
}

/// Into or out of あとで読む (docs/LATER.md).
#[tauri::command]
pub async fn set_later(state: State<'_, AppState>, ids: Vec<i64>, later: bool) -> CmdResult<()> {
    with_db(&state, |c| db::set_later(c, &ids, later))
}

/// Returns the ids it marked, for 元に戻す.
#[tauri::command]
pub async fn mark_all_read(state: State<'_, AppState>, query: ArticleQuery) -> CmdResult<Vec<i64>> {
    with_db(&state, |c| db::mark_all_read(c, &query))
}

/// An article image as a `data:` URL: the WebView never loads remote images
/// itself (the CSP has no remote `img-src`), so a feed can't track who read it
/// from the WebView, and only real images get through (`fetch::image_type`).
#[tauri::command]
pub async fn image_data_url(url: String) -> CmdResult<String> {
    use base64::Engine;
    let res = match fetch::get(
        &url,
        None,
        None,
        "image/avif,image/webp,image/*;q=0.8",
        fetch::MAX_IMAGE_BYTES,
    )
    .await?
    {
        Outcome::Fetched(r) => r,
        Outcome::NotModified => return Err("画像を取得できませんでした".into()),
    };
    let mime = fetch::image_type(&res.body).ok_or("画像ではありません")?;
    Ok(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&res.body)
    ))
}

/// Links in articles open in the OS's browser, never in the app's window.
#[tauri::command]
pub async fn open_url(url: String) -> CmdResult<()> {
    let parsed = url::Url::parse(&url).map_err(|_| format!("開けない URL です：{url}"))?;
    if !matches!(parsed.scheme(), "http" | "https" | "mailto") {
        return Err(format!("開けない URL です：{url}"));
    }
    tauri_plugin_opener::open_url(parsed.as_str(), None::<&str>).map_err(err)
}

// ------------------------------------------------------- subscriptions

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    feed_id: i64,
    title: String,
    articles: usize,
}

/// Subscribes to `url`: a feed, or a web page that names its feed.
#[tauri::command]
pub async fn add_feed(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
    folder_id: Option<i64>,
) -> CmdResult<Added> {
    let url = fetch::normalize_url(&url)?;
    if with_db(&state, |c| db::feed_id_by_url(c, &url))?.is_some() {
        return Err("このフィードはすでに購読しています".into());
    }
    let (feed_url, res, parsed) = fetch_feed_or_page(&url).await?;
    if feed_url != url && with_db(&state, |c| db::feed_id_by_url(c, &feed_url))?.is_some() {
        return Err("このページのフィードはすでに購読しています".into());
    }
    let title = parsed.title.clone().unwrap_or_else(|| host_of(&feed_url));
    let feed_id = with_db(&state, |c| {
        let id = db::add_feed(c, &feed_url, &title, parsed.site_url.as_deref(), folder_id)?;
        db::feed_fetched(
            c,
            id,
            None,
            None,
            res.etag.as_deref(),
            res.last_modified.as_deref(),
        )?;
        db::set_icon_urls(c, id, parsed.icon.as_deref(), parsed.logo.as_deref())?;
        Ok(id)
    })?;
    let articles = with_db(&state, |c| db::store_articles(c, feed_id, &parsed.articles))?;
    log::info!("subscribed {feed_url} ({articles} articles)");
    // After subscribing, in the background: the new feed's icon.
    spawn_icon_pass(&app);
    Ok(Added {
        feed_id,
        title,
        articles,
    })
}

fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| url.to_string())
}

/// The feed at `url`, or the first feed its page links to.
async fn fetch_feed_or_page(url: &str) -> CmdResult<(String, fetch::Response, feed::ParsedFeed)> {
    let res = match fetch::get(url, None, None, fetch::FEED_ACCEPT, fetch::MAX_FEED_BYTES).await? {
        Outcome::Fetched(r) => r,
        Outcome::NotModified => return Err("取得できませんでした（HTTP 304）".into()),
    };
    match feed::parse(&res.body, &res.url) {
        Ok(parsed) => Ok((res.url.clone(), res, parsed)),
        Err(parse_error) => {
            if !feed::looks_like_html(&res.body) {
                return Err(parse_error);
            }
            let candidates = feed::discover(&res.body, &res.url);
            let first = candidates
                .first()
                .ok_or("このページにはフィードが見つかりませんでした")?;
            let res = match fetch::get(first, None, None, fetch::FEED_ACCEPT, fetch::MAX_FEED_BYTES)
                .await?
            {
                Outcome::Fetched(r) => r,
                Outcome::NotModified => return Err("取得できませんでした（HTTP 304）".into()),
            };
            let parsed = feed::parse(&res.body, &res.url)?;
            Ok((first.clone(), res, parsed))
        }
    }
}

/// The feed-info dialog (docs/FEED_INFO.md).
#[tauri::command]
pub async fn get_feed_info(state: State<'_, AppState>, id: i64) -> CmdResult<db::FeedInfo> {
    with_db(&state, |c| db::feed_info(c, id))?.ok_or_else(|| "フィードが見つかりません".to_string())
}

/// Points a feed at another URL after checking it: the feed itself, or a page
/// that names it, as when subscribing. The user's name, the articles and their
/// read state stay (articles are matched by guid); the old address's etag and
/// error and the icon are cleared.
#[tauri::command]
pub async fn change_feed_url(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    url: String,
) -> CmdResult<db::FeedInfo> {
    let url = fetch::normalize_url(&url)?;
    // Another feed has it. This feed's own URL is not a conflict.
    let taken = |found: Option<i64>| found.is_some_and(|other| other != id);
    if taken(with_db(&state, |c| db::feed_id_by_url(c, &url))?) {
        return Err("このフィードはすでに購読しています".into());
    }
    let (feed_url, res, parsed) = fetch_feed_or_page(&url).await?;
    if feed_url != url && taken(with_db(&state, |c| db::feed_id_by_url(c, &feed_url))?) {
        return Err("このページのフィードはすでに購読しています".into());
    }
    let found = with_db(&state, |c| {
        if !db::change_feed_url(c, id, &feed_url)? {
            return Ok(false);
        }
        db::feed_fetched(
            c,
            id,
            parsed.title.as_deref(),
            parsed.site_url.as_deref(),
            res.etag.as_deref(),
            res.last_modified.as_deref(),
        )?;
        db::set_icon_urls(c, id, parsed.icon.as_deref(), parsed.logo.as_deref())?;
        Ok(true)
    })?;
    if !found {
        return Err("フィードが見つかりません".into());
    }
    let articles = with_db(&state, |c| db::store_articles(c, id, &parsed.articles))?;
    log::info!("feed {id} moved to {feed_url} ({articles} new articles)");
    spawn_icon_pass(&app);
    with_db(&state, |c| db::feed_info(c, id))?.ok_or_else(|| "フィードが見つかりません".to_string())
}

#[tauri::command]
pub async fn rename_feed(state: State<'_, AppState>, id: i64, title: String) -> CmdResult<()> {
    with_db(&state, |c| db::rename_feed(c, id, &title))
}

#[tauri::command]
pub async fn move_feed(
    state: State<'_, AppState>,
    id: i64,
    folder_id: Option<i64>,
) -> CmdResult<()> {
    with_db(&state, |c| db::move_feed(c, id, folder_id))
}

#[tauri::command]
pub async fn delete_feed(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_db(&state, |c| db::delete_feed(c, id))
}

#[tauri::command]
pub async fn create_folder(state: State<'_, AppState>, name: String) -> CmdResult<i64> {
    let name = name.trim();
    if name.is_empty() {
        return Err("フォルダの名前を入力してください".into());
    }
    with_db(&state, |c| db::create_folder(c, name))
}

#[tauri::command]
pub async fn rename_folder(state: State<'_, AppState>, id: i64, name: String) -> CmdResult<()> {
    let name = name.trim();
    if name.is_empty() {
        return Err("フォルダの名前を入力してください".into());
    }
    with_db(&state, |c| db::rename_folder(c, id, name))
}

#[tauri::command]
pub async fn delete_folder(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    with_db(&state, |c| db::delete_folder(c, id))
}

// --------------------------------------------------------------- refresh

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RefreshProgress {
    done: usize,
    total: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    feed_id: i64,
    title: String,
    error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSummary {
    feeds: usize,
    new_articles: usize,
    failed: Vec<Failure>,
    cancelled: bool,
}

/// Feeds fetched at a time: fast enough for a few hundred feeds, gentle
/// enough on a slow connection and on servers hosting several of them.
const PARALLEL: usize = 6;

/// Refreshes `ids` (all feeds when None). Progress goes out as
/// `refresh-progress` events; `cancel_refresh` stops it between feeds.
#[tauri::command]
pub async fn refresh_feeds(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Option<Vec<i64>>,
) -> CmdResult<RefreshSummary> {
    if state.refreshing.swap(true, Ordering::SeqCst) {
        return Err("更新中です".into());
    }
    state.cancel.store(false, Ordering::SeqCst);
    let result = refresh(&app, &state, ids).await;
    state.refreshing.store(false, Ordering::SeqCst);
    // The refresh is over for the caller; icons follow in the background.
    spawn_icon_pass(&app);
    result
}

async fn refresh(
    app: &AppHandle,
    state: &AppState,
    ids: Option<Vec<i64>>,
) -> CmdResult<RefreshSummary> {
    let targets = with_db(state, |c| db::feed_targets(c, ids.as_deref()))?;
    let total = targets.len();
    let _ = app.emit("refresh-progress", RefreshProgress { done: 0, total });
    let mut results = futures_util::stream::iter(targets.into_iter().map(|t| async move {
        if state.cancel.load(Ordering::SeqCst) {
            return (t, None);
        }
        let r = fetch::get(
            &t.url,
            t.etag.as_deref(),
            t.last_modified.as_deref(),
            fetch::FEED_ACCEPT,
            fetch::MAX_FEED_BYTES,
        )
        .await;
        (t, Some(r))
    }))
    .buffer_unordered(PARALLEL);

    let mut summary = RefreshSummary {
        feeds: total,
        new_articles: 0,
        failed: Vec::new(),
        cancelled: false,
    };
    let mut done = 0;
    while let Some((target, result)) = results.next().await {
        done += 1;
        let Some(result) = result else {
            summary.cancelled = true;
            continue;
        };
        let outcome = result.and_then(|o| match o {
            Outcome::NotModified => Ok(None),
            Outcome::Fetched(res) => feed::parse(&res.body, &res.url).map(|p| Some((res, p))),
        });
        let stored = with_db(state, |c| match &outcome {
            Ok(None) => db::feed_unchanged(c, target.id).map(|_| 0),
            Ok(Some((res, parsed))) => {
                db::feed_fetched(
                    c,
                    target.id,
                    parsed.title.as_deref(),
                    parsed.site_url.as_deref(),
                    res.etag.as_deref(),
                    res.last_modified.as_deref(),
                )?;
                db::set_icon_urls(c, target.id, parsed.icon.as_deref(), parsed.logo.as_deref())?;
                db::store_articles(c, target.id, &parsed.articles)
            }
            Err(e) => db::feed_failed(c, target.id, e).map(|_| 0),
        });
        match (outcome, stored) {
            (Err(e), _) | (_, Err(e)) => {
                log::warn!("refresh {}: {e}", target.url);
                summary.failed.push(Failure {
                    feed_id: target.id,
                    title: target.title,
                    error: e,
                });
            }
            (Ok(_), Ok(n)) => summary.new_articles += n,
        }
        let _ = app.emit("refresh-progress", RefreshProgress { done, total });
    }
    let keep_days = load_settings(app).keep_days;
    match with_db(state, |c| db::purge(c, keep_days)) {
        Ok(n) if n > 0 => log::info!("removed {n} old read articles"),
        Ok(_) => {}
        Err(e) => log::warn!("purge: {e}"),
    }
    Ok(summary)
}

#[tauri::command]
pub async fn cancel_refresh(state: State<'_, AppState>) -> CmdResult<()> {
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

// ----------------------------------------------------------------- icons

/// A feed's icon is looked up again after this long (docs/FAVICON.md).
const ICON_RECHECK_MS: i64 = 30 * 86_400_000;

/// Every feed's icon as a `data:` URL, kept apart from `get_sidebar`: that is
/// asked again after every read, and the icons would make it heavy.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedIcon {
    feed_id: i64,
    icon: String,
}

#[tauri::command]
pub async fn feed_icons(state: State<'_, AppState>) -> CmdResult<Vec<FeedIcon>> {
    let icons = with_db(&state, |c| db::icons(c))?;
    Ok(icons
        .into_iter()
        .map(|(feed_id, icon)| FeedIcon { feed_id, icon })
        .collect())
}

/// An image URL as a `data:` URL: the right type by its content (no SVG) and
/// within `MAX_ICON_BYTES` (`fetch::get` refuses more). Nothing on any failure.
async fn icon_data_url(url: &str) -> Option<String> {
    use base64::Engine;
    let res = match fetch::get(url, None, None, fetch::ICON_ACCEPT, fetch::MAX_ICON_BYTES).await {
        Ok(Outcome::Fetched(r)) => r,
        Ok(Outcome::NotModified) => return None,
        Err(e) => {
            log::debug!("icon {url}: {e}");
            return None;
        }
    };
    let mime = fetch::image_type(&res.body)?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&res.body)
    ))
}

/// How many of a page's icon links are tried before the next kind of source.
const ICON_LINKS_TRIED: usize = 3;

/// The icon for a feed, as a `data:` URL, from the first of these that gives
/// an image: (1) the feed's own icon, (2) the `<link rel="icon">` tags of the
/// site's page, (3) `/favicon.ico` of the site, (4) the feed's logo. The site
/// is the feed's `site_url`, or the feed's own address when it has none.
async fn fetch_icon(
    feed_url: &str,
    site_url: Option<&str>,
    parsed_icon: Option<&str>,
    parsed_logo: Option<&str>,
) -> Option<String> {
    if let Some(found) = match parsed_icon {
        Some(u) => icon_data_url(u).await,
        None => None,
    } {
        return Some(found);
    }
    let site = site_url
        .and_then(|u| url::Url::parse(u).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .or_else(|| url::Url::parse(feed_url).ok())
        .filter(|u| matches!(u.scheme(), "http" | "https"));
    let mut tried: Vec<String> = parsed_icon.iter().map(|u| u.to_string()).collect();
    if let Some(site) = &site {
        // The page can fail (or be missing) without the site having no icon.
        let page = fetch::get(
            site.as_str(),
            None,
            None,
            fetch::PAGE_ACCEPT,
            fetch::MAX_PAGE_BYTES,
        )
        .await;
        if let Ok(Outcome::Fetched(res)) = page {
            for link in feed::icon_links(&res.body, &res.url)
                .into_iter()
                .take(ICON_LINKS_TRIED)
            {
                if tried.contains(&link) {
                    continue;
                }
                if let Some(found) = icon_data_url(&link).await {
                    return Some(found);
                }
                tried.push(link);
            }
        }
        let favicon = format!("{}/favicon.ico", site.origin().ascii_serialization());
        if !tried.contains(&favicon) {
            if let Some(found) = icon_data_url(&favicon).await {
                return Some(found);
            }
            tried.push(favicon);
        }
    }
    match parsed_logo {
        Some(u) if !tried.iter().any(|t| t == u) => icon_data_url(u).await,
        _ => None,
    }
}

/// Looks up icons in the background (after a refresh, a subscription, a URL change).
fn spawn_icon_pass(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { icon_pass(&app).await });
}

/// One pass at a time; one asked for meanwhile runs when this one is done.
async fn icon_pass(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.icons_running.swap(true, Ordering::SeqCst) {
        state.icons_again.store(true, Ordering::SeqCst);
        return;
    }
    loop {
        state.icons_again.store(false, Ordering::SeqCst);
        look_up_icons(app, &state).await;
        if !state.icons_again.load(Ordering::SeqCst) {
            break;
        }
    }
    state.icons_running.store(false, Ordering::SeqCst);
}

/// The feeds never looked up, or not for 30 days, six at a time. A failure is
/// logged and recorded as a check (so it isn't retried at every refresh) and
/// never reaches the refresh or the subscription that started the pass.
async fn look_up_icons(app: &AppHandle, state: &AppState) {
    let cutoff = db::now_ms() - ICON_RECHECK_MS;
    let targets = match with_db(state, |c| db::feeds_needing_icon(c, cutoff)) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("icons: {e}");
            return;
        }
    };
    if targets.is_empty() {
        return;
    }
    let total = targets.len();
    let mut results = futures_util::stream::iter(targets.into_iter().map(|t| async move {
        let icon = fetch_icon(
            &t.url,
            t.site_url.as_deref(),
            t.icon_url.as_deref(),
            t.logo_url.as_deref(),
        )
        .await;
        (t.id, t.url, icon)
    }))
    .buffer_unordered(PARALLEL);
    let mut found = 0;
    while let Some((id, url, icon)) = results.next().await {
        match with_db(state, |c| db::set_icon(c, id, &url, icon.as_deref())) {
            Ok(()) => found += usize::from(icon.is_some()),
            Err(e) => log::warn!("icon {id}: {e}"),
        }
    }
    log::info!("icons: {found} of {total} feeds");
    if found > 0 {
        let _ = app.emit("icons-changed", ());
    }
}

// ------------------------------------------------------------------ OPML

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    /// The new feeds, to refresh next.
    feed_ids: Vec<i64>,
    skipped: usize,
}

/// Subscribes to the feeds in an OPML file, without fetching them yet (the
/// frontend refreshes `feed_ids` next, with progress).
#[tauri::command]
pub async fn import_opml(state: State<'_, AppState>, path: PathBuf) -> CmdResult<Imported> {
    let size = fs::metadata(&path).map_err(err)?.len();
    if size > opml::MAX_BYTES {
        return Err("OPML ファイルが大きすぎます".into());
    }
    let text = fs::read_to_string(&path).map_err(|e| format!("読み込めませんでした（{e}）"))?;
    let feeds = opml::parse(&text)?;
    with_db(&state, |c| {
        let tx = c.transaction()?;
        let mut feed_ids = Vec::new();
        let mut skipped = 0;
        for f in &feeds {
            let Ok(url) = fetch::normalize_url(&f.url) else {
                skipped += 1;
                continue;
            };
            if db::feed_id_by_url(&tx, &url)?.is_some() {
                skipped += 1;
                continue;
            }
            let folder = f
                .folder
                .as_deref()
                .map(|n| db::folder_named(&tx, n))
                .transpose()?;
            feed_ids.push(db::add_feed(
                &tx,
                &url,
                &f.title,
                f.site_url.as_deref(),
                folder,
            )?);
        }
        tx.commit()?;
        Ok(Imported { feed_ids, skipped })
    })
}

#[tauri::command]
pub async fn export_opml(state: State<'_, AppState>, path: PathBuf) -> CmdResult<usize> {
    let feeds = with_db(&state, |c| db::feeds_for_export(c))?;
    fs::write(&path, opml::write(&feeds)).map_err(|e| format!("書き出せませんでした（{e}）"))?;
    Ok(feeds.len())
}

// ------------------------------------------------------------------- app

/// Help › ログを表示: the log file in Finder / Explorer.
#[tauri::command]
pub async fn reveal_log(app: AppHandle) -> CmdResult<()> {
    let dir = app.path().app_log_dir().map_err(err)?;
    let file = dir.join(format!("{}.log", crate::LOG_FILE));
    if file.is_file() {
        tauri_plugin_opener::reveal_item_in_dir(file).map_err(err)
    } else {
        fs::create_dir_all(&dir).map_err(err)?;
        tauri_plugin_opener::open_path(dir, None::<&str>).map_err(err)
    }
}

/// The OS's proxy for the update feed (src/lib/update.ts): the updater's
/// HTTP client can't run PAC scripts.
#[tauri::command]
pub async fn system_proxy(url: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || crate::proxy::for_url(&url))
        .await
        .ok()
        .flatten()
}
