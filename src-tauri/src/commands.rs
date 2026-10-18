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
use crate::{extension, feed, fulltext, markdown, opened, opml, sanitize, stockroom};
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
    /// The same for the 全文取得 pass.
    full_running: AtomicBool,
    full_again: AtomicBool,
    /// What the OS handed over to open, until the window takes it (it may
    /// not be loaded yet at launch). docs/OPEN_WITH.md.
    opened: Mutex<Vec<opened::OpenRequest>>,
    /// The browser extension's receiver while it runs, or why it doesn't.
    extension: Mutex<ExtensionServer>,
    /// Pairing questions open in the window: their answer goes here.
    extension_pairing: Mutex<std::collections::HashMap<String, std::sync::mpsc::Sender<bool>>>,
}

#[derive(Default)]
struct ExtensionServer {
    server: Option<extension::Server>,
    error: Option<String>,
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
    /// When opening an article marks it read (docs/READING.md).
    mark_read_on: MarkReadOn,
}

#[derive(Clone, Copy, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MarkReadOn {
    /// As soon as it is opened.
    #[default]
    Open,
    /// Two seconds after it is opened.
    Delay,
    /// Only by hand.
    Manual,
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
            mark_read_on: MarkReadOn::Open,
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

/// settings.json as a whole: the settings dialog's part, and stockroom's,
/// which never goes to the window (it holds the token).
#[derive(Clone, Default, Serialize, Deserialize)]
struct SettingsFile {
    #[serde(flatten)]
    app: AppSettings,
    #[serde(default)]
    stockroom: StockroomSettings,
    #[serde(default)]
    extension: ExtensionSettings,
}

/// The browser extension's receiver (docs/EXTENSION.md); off by default.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ExtensionSettings {
    enabled: bool,
    /// Made when the receiver is first turned on; never sent to the window.
    token: String,
}

/// Where articles go in stockroom, and the key to it (docs/STOCKROOM.md).
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct StockroomSettings {
    /// From stockroom's connection question; None: not connected.
    token: Option<String>,
    /// stockroom's folder id; None: 未分類.
    folder: Option<String>,
    /// The folder's name when it was chosen, for the settings to show
    /// without asking stockroom.
    folder_name: Option<String>,
    tags: Vec<String>,
}

fn load_settings(app: &AppHandle) -> AppSettings {
    load_settings_file(app).app
}

fn load_settings_file(app: &AppHandle) -> SettingsFile {
    match settings_path(app) {
        Ok(path) => load_settings_at(&path),
        Err(_) => SettingsFile::default(),
    }
}

fn load_settings_at(path: &std::path::Path) -> SettingsFile {
    let Ok(bytes) = fs::read(path) else {
        return SettingsFile::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_else(|e| {
        // Kept aside rather than overwritten by the next save.
        log::warn!("{}: 読み込めないため退避します: {e}", path.display());
        let _ = fs::rename(
            path,
            path.with_extension(format!("json.bad-{}", db::now_ms())),
        );
        SettingsFile::default()
    })
}

/// Changes settings.json: read, changed by `f`, written back, one change at
/// a time. The settings dialog and stockroom's part are saved by separate
/// commands that can run at once; each must start from the other's latest
/// write, or the last one would bring back what the other had just changed.
fn update_settings_file<T>(
    app: &AppHandle,
    f: impl FnOnce(&mut SettingsFile) -> T,
) -> CmdResult<T> {
    update_settings_at(&settings_path(app)?, f)
}

fn update_settings_at<T>(
    path: &std::path::Path,
    f: impl FnOnce(&mut SettingsFile) -> T,
) -> CmdResult<T> {
    static WRITING: Mutex<()> = Mutex::new(());
    let _one = WRITING.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = load_settings_at(path);
    let out = f(&mut file);
    // Written to a temporary file and renamed over the old one, so a crash
    // mid-write never leaves half a file.
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(err)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&file).map_err(err)?).map_err(err)?;
    fs::rename(&tmp, path).map_err(err)?;
    Ok(out)
}

#[tauri::command]
pub async fn get_app_settings(app: AppHandle) -> AppSettings {
    load_settings(&app)
}

/// stockroom's part of the file stays as it is.
#[tauri::command]
pub async fn set_app_settings(app: AppHandle, settings: AppSettings) -> CmdResult<AppSettings> {
    if let Some(w) = app.get_webview_window("main") {
        apply_theme(&w, settings.theme);
    }
    update_settings_file(&app, |file| file.app = settings.clone())?;
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
    /// Where `html` comes from (全文取得, docs/FULL_TEXT.md).
    full_text: FullText,
}

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum FullText {
    /// The feed's own text (全文取得 is off, or the page gave nothing).
    Feed,
    /// The text from the article's page.
    Page,
    /// The feed's text for now: the page is to be fetched (`fetch_full_text`).
    Pending,
}

#[tauri::command]
pub async fn get_article(state: State<'_, AppState>, id: i64) -> CmdResult<Article> {
    let a = with_db(&state, |c| db::article(c, id))?.ok_or("記事が見つかりません")?;
    Ok(reader_article(a))
}

/// The article's text as the reader shows it, sanitized: the page's (全文取得)
/// when there is one, else the feed's content, else its summary.
fn shown_html(a: &db::ArticleFull) -> (String, FullText) {
    let page = a
        .full_content
        .as_deref()
        .filter(|_| a.full_text && a.full_status == Some(db::FullStatus::Fetched));
    let full_text = if page.is_some() {
        FullText::Page
    } else if page_pending(a) {
        FullText::Pending
    } else {
        FullText::Feed
    };
    let base = a.url.as_deref().or(a.site_url.as_deref());
    let source = match page {
        Some(p) => p,
        None if a.content.trim().is_empty() => &a.summary,
        None => &a.content,
    };
    (sanitize::article_html(source, base), full_text)
}

fn reader_article(a: db::ArticleFull) -> Article {
    let (html, full_text) = shown_html(&a);
    Article {
        html,
        full_text,
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
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticleMarkdown {
    /// `<title>.md`, safe as a file name.
    name: String,
    markdown: String,
}

/// The article as one Markdown file (docs/STOCKROOM.md): what the reader
/// shows, with a header of where it came from. `utc_offset_minutes` (east
/// positive) dates it in the user's time zone.
#[tauri::command]
pub async fn article_markdown(
    state: State<'_, AppState>,
    id: i64,
    utc_offset_minutes: i32,
) -> CmdResult<ArticleMarkdown> {
    let a = with_db(&state, |c| db::article(c, id))?.ok_or("記事が見つかりません")?;
    Ok(to_markdown(&a, utc_offset_minutes))
}

fn to_markdown(a: &db::ArticleFull, utc_offset_minutes: i32) -> ArticleMarkdown {
    let (html, _) = shown_html(a);
    let meta = markdown::Meta {
        title: &a.title,
        url: a.url.as_deref(),
        feed_title: &a.feed_title,
        published: a.published,
        saved: db::now_ms(),
        utc_offset_minutes,
    };
    let (name, markdown) = markdown::article(&meta, &html);
    ArticleMarkdown { name, markdown }
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
    let (body, mime) = fetch_image(&url).await?;
    Ok(data_url(mime, &body))
}

/// The picture of a row in the list's 画像付き layout: an article image as
/// `image_data_url` gets it, made small first (thumb.rs) off the async threads.
#[tauri::command]
pub async fn thumbnail_data_url(url: String) -> CmdResult<String> {
    let (body, mime) = fetch_image(&url).await?;
    let (mime, small) =
        tauri::async_runtime::spawn_blocking(move || crate::thumb::make(&body, mime))
            .await
            .map_err(err)??;
    Ok(data_url(mime, &small))
}

/// An image's bytes and type, checked by its first bytes.
async fn fetch_image(url: &str) -> CmdResult<(Vec<u8>, &'static str)> {
    let res = match fetch::get(
        url,
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
    Ok((res.body, mime))
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    use base64::Engine;
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
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
    subscribe(&app, &state, &url, folder_id).await
}

/// Subscribes to `url` (a feed, or a page naming one): from the add-feed
/// dialog and from the browser extension.
async fn subscribe(
    app: &AppHandle,
    state: &AppState,
    url: &str,
    folder_id: Option<i64>,
) -> CmdResult<Added> {
    let url = fetch::normalize_url(url)?;
    if with_db(state, |c| db::feed_id_by_url(c, &url))?.is_some() {
        return Err("このフィードはすでに購読しています".into());
    }
    let (feed_url, res, parsed) = fetch_feed_or_page(&url).await?;
    if feed_url != url && with_db(state, |c| db::feed_id_by_url(c, &feed_url))?.is_some() {
        return Err("このページのフィードはすでに購読しています".into());
    }
    let title = parsed.title.clone().unwrap_or_else(|| host_of(&feed_url));
    let feed_id = with_db(state, |c| {
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
    let articles = with_db(state, |c| db::store_articles(c, feed_id, &parsed.articles))?;
    log::info!("subscribed {feed_url} ({articles} articles)");
    // After subscribing, in the background: the new feed's icon.
    spawn_icon_pass(app);
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
    // The refresh is over for the caller; icons and the pages of 全文取得
    // follow in the background.
    spawn_icon_pass(&app);
    spawn_full_text_pass(&app);
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

// -------------------------------------------------------------- stockroom

/// What the settings show of the connection to stockroom (no token).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockroomStatus {
    connected: bool,
    folder: Option<String>,
    folder_name: Option<String>,
    tags: Vec<String>,
}

fn stockroom_status_of(s: &StockroomSettings) -> StockroomStatus {
    StockroomStatus {
        connected: s.token.is_some(),
        folder: s.folder.clone(),
        folder_name: s.folder_name.clone(),
        tags: s.tags.clone(),
    }
}

/// Whether stockroom is connected and where articles go. Asks stockroom nothing.
#[tauri::command]
pub async fn stockroom_status(app: AppHandle) -> StockroomStatus {
    stockroom_status_of(&load_settings_file(&app).stockroom)
}

/// Asks stockroom to connect, showing `code` (the window shows it too);
/// waits while the user answers in stockroom.
#[tauri::command]
pub async fn stockroom_pair(app: AppHandle, code: String) -> CmdResult<StockroomStatus> {
    if code.len() != 4 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err("確認用の番号が正しくありません".into());
    }
    let token = stockroom::pair(stockroom::BASE, &code)
        .await
        .map_err(stockroom_message)?;
    update_settings_file(&app, |file| {
        file.stockroom.token = Some(token);
        stockroom_status_of(&file.stockroom)
    })
}

/// Forgets the token (stockroom keeps allowing it until its token is made again there).
#[tauri::command]
pub async fn stockroom_disconnect(app: AppHandle) -> CmdResult<StockroomStatus> {
    update_settings_file(&app, |file| {
        file.stockroom.token = None;
        stockroom_status_of(&file.stockroom)
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StockroomChoices {
    folders: Vec<stockroom::Folder>,
    tags: Vec<String>,
}

/// stockroom's folders and tags, for choosing where articles go (asked when
/// the settings show them).
#[tauri::command]
pub async fn stockroom_choices(app: AppHandle) -> CmdResult<StockroomChoices> {
    let token = stockroom_token(&app)?;
    let folders = stockroom::folders(stockroom::BASE, &token).await;
    let folders = forget_if_unauthorized(&app, folders)?;
    let tags = stockroom::tags(stockroom::BASE, &token).await;
    let tags = forget_if_unauthorized(&app, tags)?;
    Ok(StockroomChoices { folders, tags })
}

#[tauri::command]
pub async fn set_stockroom_target(
    app: AppHandle,
    folder: Option<String>,
    folder_name: Option<String>,
    tags: Vec<String>,
) -> CmdResult<StockroomStatus> {
    let mut clean: Vec<String> = Vec::new();
    for t in tags.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        if !clean.iter().any(|c| c == t) {
            clean.push(t.to_string());
        }
    }
    update_settings_file(&app, |file| {
        file.stockroom.folder = folder;
        file.stockroom.folder_name = folder_name;
        file.stockroom.tags = clean;
        stockroom_status_of(&file.stockroom)
    })
}

/// The window tells this from any other error: it asks the user to connect.
const NOT_CONNECTED: &str = "stockroom-not-connected";

fn stockroom_token(app: &AppHandle) -> CmdResult<String> {
    load_settings_file(app)
        .stockroom
        .token
        .ok_or_else(|| NOT_CONNECTED.to_string())
}

fn stockroom_message(e: stockroom::Error) -> String {
    match e {
        stockroom::Error::Unauthorized => NOT_CONNECTED.into(),
        stockroom::Error::Other(s) => s,
    }
}

/// A token stockroom no longer takes (made again there) is forgotten, so the
/// next try asks to connect again.
fn forget_if_unauthorized<T>(app: &AppHandle, r: stockroom::Result<T>) -> CmdResult<T> {
    if r.as_ref()
        .is_err_and(|e| *e == stockroom::Error::Unauthorized)
    {
        update_settings_file(app, |file| file.stockroom.token = None)?;
    }
    r.map_err(stockroom_message)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    /// stockroom already had the same text.
    duplicate: bool,
}

/// The article as Markdown into stockroom, to the folder and tags of the
/// settings. Fails with `NOT_CONNECTED` when there is no good token.
#[tauri::command]
pub async fn send_to_stockroom(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    utc_offset_minutes: i32,
) -> CmdResult<Sent> {
    let settings = load_settings_file(&app).stockroom;
    let token = stockroom_token(&app)?;
    let a = with_db(&state, |c| db::article(c, id))?.ok_or("記事が見つかりません")?;
    let md = to_markdown(&a, utc_offset_minutes);
    forget_if_unauthorized(&app, stockroom::check(stockroom::BASE, &token).await)?;
    let item = stockroom::Item {
        name: &md.name,
        markdown: &md.markdown,
        page: a.url.as_deref(),
        folder: settings.folder.as_deref(),
        tags: &settings.tags,
    };
    let duplicate = forget_if_unauthorized(
        &app,
        stockroom::import(stockroom::BASE, &token, &item).await,
    )?;
    Ok(Sent { duplicate })
}

// ------------------------------------------------------ browser extension

/// The app as the extension's receiver sees it (extension.rs `Host`).
struct ExtensionHost(AppHandle);

/// Something came in from the browser (the `extension-changed` event): the
/// window says what and reloads, and when the extension asked for it
/// (`show=1`), comes to the front at it.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FromBrowser {
    message: String,
    /// The feed subscribed, to show.
    feed_id: Option<i64>,
    /// The page put into あとで読む, to show.
    later_id: Option<i64>,
}

fn tell_window(app: &AppHandle, news: FromBrowser) {
    if news.feed_id.is_some() || news.later_id.is_some() {
        crate::show_main_window(app);
    }
    let _ = app.emit("extension-changed", news);
}

impl extension::Host for ExtensionHost {
    fn subscribe(&self, url: &str, options: extension::Options) -> Result<String, String> {
        let app = &self.0;
        let state = app.state::<AppState>();
        let added = tauri::async_runtime::block_on(subscribe(app, &state, url, None))?;
        if options.full_text {
            with_db(&state, |c| db::set_full_text(c, added.feed_id, true))?;
        }
        tell_window(
            app,
            FromBrowser {
                message: format!(
                    "「{}」を購読しました（ブラウザから{}）",
                    added.title,
                    if options.full_text {
                        "・全文取得オン"
                    } else {
                        ""
                    }
                ),
                feed_id: options.show.then_some(added.feed_id),
                later_id: None,
            },
        );
        Ok(added.title)
    }

    fn save_page(
        &self,
        url: &str,
        title: &str,
        html: &str,
        options: extension::Options,
    ) -> Result<(), String> {
        let app = &self.0;
        // Nothing found (a page drawn by JavaScript): the title and the link stay.
        let body = fulltext::extract(html, url).unwrap_or_default();
        let page = feed::page_article(url, title, &body, db::now_ms());
        let id = with_db(&app.state::<AppState>(), |c| db::save_page(c, &page))?;
        tell_window(
            app,
            FromBrowser {
                message: format!("「{}」をあとで読むに入れました", page.title),
                feed_id: None,
                later_id: options.show.then_some(id),
            },
        );
        Ok(())
    }

    fn approve_pairing(&self, code: &str, extension: &str) -> bool {
        #[derive(Clone, Serialize)]
        struct Ask<'a> {
            id: &'a str,
            code: &'a str,
            extension: &'a str,
        }
        let app = &self.0;
        let state = app.state::<AppState>();
        let id = extension::new_token()[..16].to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        state
            .extension_pairing
            .lock()
            .unwrap()
            .insert(id.clone(), tx);
        crate::show_main_window(app);
        let _ = app.emit(
            "extension-pair",
            Ask {
                id: &id,
                code,
                extension,
            },
        );
        let approved = rx
            .recv_timeout(std::time::Duration::from_secs(120))
            .unwrap_or(false);
        state.extension_pairing.lock().unwrap().remove(&id);
        let _ = app.emit("extension-pair-end", &id);
        approved
    }
}

/// The user's answer to "connect this extension?".
#[tauri::command]
pub async fn answer_extension_pair(
    state: State<'_, AppState>,
    id: String,
    allow: bool,
) -> CmdResult<()> {
    if let Some(tx) = state.extension_pairing.lock().unwrap().remove(&id) {
        let _ = tx.send(allow);
    }
    Ok(())
}

/// Starts or stops the receiver to match the settings (a restart picks up a new token).
fn apply_extension(app: &AppHandle) {
    let state = app.state::<AppState>();
    let mut ext = state.extension.lock().unwrap();
    *ext = ExtensionServer::default(); // stops a running one first, freeing the port
    let s = load_settings_file(app).extension;
    if s.enabled && !s.token.is_empty() {
        match extension::Server::start(
            extension::PORT,
            s.token,
            std::sync::Arc::new(ExtensionHost(app.clone())),
        ) {
            Ok((server, _)) => ext.server = Some(server),
            Err(e) => {
                log::warn!("browser extension receiver: {e}");
                ext.error = Some(e);
            }
        }
    }
}

/// At launch, if it was left on.
pub fn start_extension_receiver(app: &AppHandle) {
    if load_settings_file(app).extension.enabled {
        apply_extension(app);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionStatus {
    enabled: bool,
    running: bool,
    port: u16,
    /// Why it isn't running although it's on.
    error: Option<String>,
    /// Where `install_extension` put the extension, if it has.
    folder: Option<String>,
}

fn extension_folder(app: &AppHandle) -> CmdResult<PathBuf> {
    Ok(app.path().app_data_dir().map_err(err)?.join("extension"))
}

fn extension_status_of(app: &AppHandle) -> CmdResult<ExtensionStatus> {
    let enabled = load_settings_file(app).extension.enabled;
    let state = app.state::<AppState>();
    let ext = state.extension.lock().unwrap();
    let dir = extension_folder(app)?;
    Ok(ExtensionStatus {
        enabled,
        running: ext.server.is_some(),
        port: extension::PORT,
        error: ext.error.clone(),
        folder: dir
            .join("manifest.json")
            .is_file()
            .then(|| dir.display().to_string()),
    })
}

#[tauri::command]
pub async fn extension_status(app: AppHandle) -> CmdResult<ExtensionStatus> {
    extension_status_of(&app)
}

fn update_extension_settings<T>(
    app: &AppHandle,
    f: impl FnOnce(&mut ExtensionSettings) -> T,
) -> CmdResult<T> {
    update_settings_file(app, |file| f(&mut file.extension))
}

#[tauri::command]
pub async fn set_extension_receiver(app: AppHandle, enabled: bool) -> CmdResult<ExtensionStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        update_extension_settings(&app, |s| {
            s.enabled = enabled;
            if s.token.is_empty() {
                s.token = extension::new_token();
            }
        })?;
        apply_extension(&app);
        extension_status_of(&app)
    })
    .await
    .map_err(err)?
}

/// A new token: every extension connected before is cut off until it
/// connects again (the copy written out here gets the new one).
#[tauri::command]
pub async fn reset_extension_token(app: AppHandle) -> CmdResult<ExtensionStatus> {
    tauri::async_runtime::spawn_blocking(move || {
        let token = update_extension_settings(&app, |s| {
            s.token = extension::new_token();
            s.token.clone()
        })?;
        let dir = extension_folder(&app)?;
        if dir.join("manifest.json").is_file() {
            write_extension_config(&dir, &token)?;
        }
        apply_extension(&app);
        extension_status_of(&app)
    })
    .await
    .map_err(err)?
}

/// Tells the written-out copy where and how to connect.
fn write_extension_config(dir: &std::path::Path, token: &str) -> CmdResult<()> {
    let config = serde_json::json!({ "port": extension::PORT, "token": token });
    fs::write(
        dir.join("config.json"),
        serde_json::to_vec_pretty(&config).map_err(err)?,
    )
    .map_err(err)
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Copies the extension the app carries to a folder Chrome can load, with
/// the token in its config.json, and shows the folder. Returns it.
#[tauri::command]
pub async fn install_extension(app: AppHandle) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let src = app.path().resource_dir().map_err(err)?.join("extension");
        if !src.join("manifest.json").is_file() {
            return Err(format!(
                "拡張機能のファイルが見つかりません: {}",
                src.display()
            ));
        }
        let token = update_extension_settings(&app, |s| {
            if s.token.is_empty() {
                s.token = extension::new_token();
            }
            s.token.clone()
        })?;
        let dir = extension_folder(&app)?;
        let _ = fs::remove_dir_all(&dir);
        copy_dir(&src, &dir).map_err(err)?;
        write_extension_config(&dir, &token)?;
        let _ = tauri_plugin_opener::reveal_item_in_dir(&dir);
        Ok(dir.display().to_string())
    })
    .await
    .map_err(err)?
}

// ------------------------------------------------------------ open with

/// Keeps what the OS handed over and tells the window (`open-requested`),
/// which takes it with `take_open_requests`.
pub fn hand_over(app: &AppHandle, requests: Vec<opened::OpenRequest>) {
    if requests.is_empty() {
        return;
    }
    log::info!("opened: {requests:?}");
    app.state::<AppState>()
        .opened
        .lock()
        .unwrap()
        .extend(requests);
    let _ = app.emit("open-requested", ());
}

#[tauri::command]
pub async fn take_open_requests(state: State<'_, AppState>) -> CmdResult<Vec<opened::OpenRequest>> {
    Ok(std::mem::take(&mut *state.opened.lock().unwrap()))
}

// --------------------------------------------------------------- 全文取得

/// Articles fetched from one feed after a refresh; older new ones are
/// fetched when opened (docs/FULL_TEXT.md).
const FULL_PER_FEED: usize = 20;

#[tauri::command]
pub async fn set_feed_full_text(state: State<'_, AppState>, id: i64, on: bool) -> CmdResult<()> {
    with_db(&state, |c| db::set_full_text(c, id, on))
}

/// The open article's page, fetched now (`FullText::Pending`). On failure
/// the article comes back with the feed's text, and isn't tried again.
#[tauri::command]
pub async fn fetch_full_text(state: State<'_, AppState>, id: i64) -> CmdResult<Article> {
    let a = with_db(&state, |c| db::article(c, id))?.ok_or("記事が見つかりません")?;
    if page_pending(&a) {
        let content = page_article(a.url.as_deref().unwrap_or_default()).await;
        with_db(&state, |c| db::set_full_content(c, id, content.as_deref()))?;
    }
    let a = with_db(&state, |c| db::article(c, id))?.ok_or("記事が見つかりません")?;
    Ok(reader_article(a))
}

/// The feed has 全文取得 on and the article's page hasn't been tried yet.
fn page_pending(a: &db::ArticleFull) -> bool {
    a.full_text && a.url.is_some() && matches!(a.full_status, None | Some(db::FullStatus::Queued))
}

/// The article's HTML from its page, or None (logged) when there is none.
async fn page_article(url: &str) -> Option<String> {
    let res = match fetch::get_page(url).await {
        Ok(res) => res,
        Err(e) => {
            log::warn!("full text {url}: {e}");
            return None;
        }
    };
    let out = tauri::async_runtime::spawn_blocking(move || {
        let html = fulltext::decode(&res.body, res.content_type.as_deref());
        fulltext::extract(&html, &res.url)
    })
    .await
    .ok()
    .flatten();
    if out.is_none() {
        log::warn!("full text {url}: no article found");
    }
    out
}

fn spawn_full_text_pass(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        if state.full_running.swap(true, Ordering::SeqCst) {
            state.full_again.store(true, Ordering::SeqCst);
            return;
        }
        loop {
            state.full_again.store(false, Ordering::SeqCst);
            fetch_queued_pages(&app, &state).await;
            if !state.full_again.load(Ordering::SeqCst) {
                break;
            }
        }
        state.full_running.store(false, Ordering::SeqCst);
    });
}

/// The queued articles' pages: one at a time per host (a feed's articles are
/// mostly on one site), up to `PARALLEL` hosts at once. Each one done goes
/// out as `full-text` with the article's id, for the reader showing it.
async fn fetch_queued_pages(app: &AppHandle, state: &AppState) {
    let targets = match with_db(state, |c| db::take_full_queue(c, FULL_PER_FEED)) {
        Ok(t) => t,
        Err(e) => {
            log::warn!("full text: {e}");
            return;
        }
    };
    if targets.is_empty() {
        return;
    }
    let mut by_host: Vec<(String, Vec<db::FullTarget>)> = Vec::new();
    for t in targets {
        let host = t.url.as_deref().map(host_of).unwrap_or_default();
        match by_host.iter_mut().find(|(h, _)| *h == host) {
            Some((_, list)) => list.push(t),
            None => by_host.push((host, vec![t])),
        }
    }
    let total: usize = by_host.iter().map(|(_, l)| l.len()).sum();
    let fetched = futures_util::stream::iter(by_host.into_iter().map(|(_, list)| async move {
        let mut fetched = 0;
        for t in list {
            let content = match &t.url {
                Some(url) => page_article(url).await,
                None => None,
            };
            match with_db(state, |c| db::set_full_content(c, t.id, content.as_deref())) {
                Ok(()) => {
                    fetched += usize::from(content.is_some());
                    let _ = app.emit("full-text", t.id);
                }
                Err(e) => log::warn!("full text {}: {e}", t.id),
            }
        }
        fetched
    }))
    .buffer_unordered(PARALLEL)
    .fold(0, |n, f| async move { n + f })
    .await;
    log::info!("full text: {fetched} of {total} articles");
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
/// This version's release page on GitHub, in the browser: the update dialog
/// and the settings' アップデート section link there for what changed,
/// instead of showing the notes in the app.
#[tauri::command]
pub async fn open_release_page(version: String) -> CmdResult<()> {
    tauri_plugin_opener::open_url(release_page(&version)?, None::<&str>).map_err(err)
}

/// Same repository as `plugins.updater.endpoints` in tauri.conf.json. Only a
/// version goes into the URL, so the page can't be sent anywhere else.
fn release_page(version: &str) -> CmdResult<String> {
    let version = version.strip_prefix('v').unwrap_or(version);
    let ok = !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    if !ok {
        return Err(format!("not a version: {version}"));
    }
    Ok(format!(
        "https://github.com/hamirilo/feedability/releases/tag/v{version}"
    ))
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_without_mark_read_on_load_as_open() {
        let s: AppSettings = serde_json::from_str(r#"{"autoUpdate":false,"keepDays":7}"#).unwrap();
        assert_eq!(s.mark_read_on, MarkReadOn::Open);
        assert!(!s.auto_update);
        let s: AppSettings = serde_json::from_str(r#"{"markReadOn":"delay"}"#).unwrap();
        assert_eq!(s.mark_read_on, MarkReadOn::Delay);
    }

    #[test]
    fn settings_written_at_once_keep_both_changes() {
        let dir = std::env::temp_dir().join(format!("feedability-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        // The settings dialog and stockroom's part, saved at the same time over and over.
        let app = {
            let path = path.clone();
            std::thread::spawn(move || {
                for days in 1..=50 {
                    update_settings_at(&path, |f| f.app.keep_days = days).unwrap();
                }
            })
        };
        let stockroom = {
            let path = path.clone();
            std::thread::spawn(move || {
                for n in 1..=50 {
                    update_settings_at(&path, |f| f.stockroom.tags.push(format!("t{n}"))).unwrap();
                }
            })
        };
        app.join().unwrap();
        stockroom.join().unwrap();
        let f = load_settings_at(&path);
        assert_eq!(f.app.keep_days, 50);
        assert_eq!(f.stockroom.tags.len(), 50);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn release_pages_are_only_versions() {
        assert_eq!(
            release_page("0.5.0").unwrap(),
            "https://github.com/hamirilo/feedability/releases/tag/v0.5.0"
        );
        assert_eq!(
            release_page("v0.5.0").unwrap(),
            "https://github.com/hamirilo/feedability/releases/tag/v0.5.0"
        );
        assert!(release_page("").is_err());
        assert!(release_page("0.5.0/../../evil").is_err());
        assert!(release_page("0.5.0?x=1").is_err());
    }

    #[test]
    fn stockroom_settings_sit_beside_the_others_and_stay_out_of_the_window() {
        // A file from before: no stockroom part.
        let f: SettingsFile = serde_json::from_str(r#"{"keepDays":7}"#).unwrap();
        assert_eq!(f.app.keep_days, 7);
        assert!(f.stockroom.token.is_none() && f.stockroom.tags.is_empty());

        let mut f = f;
        f.stockroom.token = Some("t0k".into());
        f.stockroom.tags = vec!["rss".into()];
        let text = serde_json::to_string(&f).unwrap();
        let back: SettingsFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.stockroom.token.as_deref(), Some("t0k"));
        assert_eq!(back.app.keep_days, 7);
        // What the window gets has no token.
        assert!(!serde_json::to_string(&back.app).unwrap().contains("t0k"));
    }
}
