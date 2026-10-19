/**
 * Typed wrappers for the Rust commands (src-tauri/src/commands.rs). The rest
 * of the frontend calls Rust only through these; src/dev/mockBackend.ts
 * answers the same commands in a plain browser.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type View =
  | { kind: "all" }
  | { kind: "starred" }
  | { kind: "later" }
  | { kind: "feed"; id: number }
  | { kind: "folder"; id: number };

export interface FolderRow {
  id: number;
  name: string;
  unread: number;
}

export interface FeedRow {
  id: number;
  title: string;
  url: string;
  siteUrl: string | null;
  folderId: number | null;
  unread: number;
  /** Why the last refresh failed; null when it worked. */
  error: string | null;
  fetchedAt: number | null;
  /** 全文取得 is on (docs/FULL_TEXT.md). */
  fullText: boolean;
}

export interface Sidebar {
  folders: FolderRow[];
  feeds: FeedRow[];
  unread: number;
  starred: number;
  later: number;
}

/** What the feed-info dialog shows (db.rs `FeedInfo`). */
export interface FeedInfo {
  id: number;
  /** The name shown: the user's, else the feed's own. */
  title: string;
  /** The feed's own title. */
  originalTitle: string;
  customTitle: string | null;
  url: string;
  siteUrl: string | null;
  folderId: number | null;
  addedAt: number;
  fetchedAt: number | null;
  /** Why the last refresh failed; null when it worked. */
  error: string | null;
  articles: number;
  unread: number;
  fullText: boolean;
}

export interface ArticleQuery {
  view: View;
  unreadOnly: boolean;
  search: string;
  /** Listed even when read (the article open in the reader). */
  keep?: number | null;
  /** Oldest first (not in あとで読む). */
  oldestFirst?: boolean;
}

export interface ArticleRow {
  id: number;
  feedId: number;
  feedTitle: string;
  title: string;
  summary: string;
  url: string | null;
  published: number;
  read: boolean;
  starred: boolean;
  /** In あとで読む. */
  later: boolean;
  /** The picture for the 画像付き layout (`thumbnailDataUrl`); null: none. */
  thumbUrl: string | null;
}

export interface Article {
  id: number;
  feedId: number;
  feedTitle: string;
  siteUrl: string | null;
  title: string;
  url: string | null;
  author: string | null;
  /** Sanitized in Rust; images still point at their servers (Reader.tsx). */
  html: string;
  published: number;
  read: boolean;
  starred: boolean;
  later: boolean;
  /** Where `html` comes from: the feed, the article's page (全文取得), or the feed for now, the page to be fetched (`fetchFullText`). */
  fullText: FullTextSource;
}

export type FullTextSource = "feed" | "page" | "pending";

/** The connection to stockroom as the settings show it (docs/STOCKROOM.md); the token stays in Rust. */
export interface StockroomStatus {
  connected: boolean;
  /** stockroom's folder id; null: 未分類. */
  folder: string | null;
  folderName: string | null;
  tags: string[];
}

export interface StockroomChoices {
  folders: { id: string; name: string; parentId: string | null }[];
  tags: string[];
}

/** The browser extension's receiver (docs/EXTENSION.md). */
export interface ExtensionStatus {
  enabled: boolean;
  running: boolean;
  port: number;
  /** Why it isn't running although it's on. */
  error: string | null;
  /** Where the extension was written out, if it was. */
  folder: string | null;
}

/** An extension asks to connect (the "extension-pair" event). */
export interface ExtensionPairRequest {
  id: string;
  code: string;
  extension: string;
}

/** Something came in from the browser extension (the "extension-changed" event). */
export interface FromBrowser {
  /** What to say. */
  message: string;
  /** The extension asked to show it: the feed subscribed… */
  feedId: number | null;
  /** …or the page put into あとで読む. */
  laterId: number | null;
}

/** What the OS handed over to open (docs/OPEN_WITH.md): a `feed:` link's feed, or an .opml file. */
export type OpenRequest = { kind: "feed"; url: string } | { kind: "opml"; path: string };

/** The error of a stockroom command when there is no good token: ask to connect. */
export const STOCKROOM_NOT_CONNECTED = "stockroom-not-connected";

const utcOffsetMinutes = () => -new Date().getTimezoneOffset();

/** An article as one Markdown file (docs/STOCKROOM.md). */
export interface ArticleMarkdown {
  /** `<title>.md`, safe as a file name. */
  name: string;
  markdown: string;
}

export type ThemeSetting = "system" | "light" | "dark";

export type MarkReadOn = "open" | "delay" | "manual";

export interface AppSettings {
  autoUpdate: boolean;
  /** 0 = only by hand. */
  refreshMinutes: number;
  /** 0 = keep read articles for good. */
  keepDays: number;
  theme: ThemeSetting;
  /** After すべて既読, open the next feed with unread articles. */
  nextFeedAfterReadAll: boolean;
  /** When opening an article marks it read. */
  markReadOn: MarkReadOn;
  /** 表示の大きさ: the whole page's zoom in percent (docs/UI_SCALE.md). */
  uiScale: number;
}

export interface Added {
  feedId: number;
  title: string;
  articles: number;
}

export interface RefreshSummary {
  feeds: number;
  newArticles: number;
  failed: { feedId: number; title: string; error: string }[];
  cancelled: boolean;
}

export interface RefreshProgress {
  done: number;
  total: number;
}

export interface FeedIcon {
  feedId: number;
  /** A data: URL. */
  icon: string;
}

export interface Imported {
  feedIds: number[];
  skipped: number;
}

export const api = {
  getSidebar: () => invoke<Sidebar>("get_sidebar"),
  queryArticles: (query: ArticleQuery) => invoke<ArticleRow[]>("query_articles", { query }),
  getArticle: (id: number) => invoke<Article>("get_article", { id }),
  setRead: (ids: number[], read: boolean) => invoke<void>("set_read", { ids, read }),
  setStarred: (id: number, starred: boolean) => invoke<void>("set_starred", { id, starred }),
  setLater: (ids: number[], later: boolean) => invoke<void>("set_later", { ids, later }),
  /** Returns the ids it marked, for 元に戻す. */
  markAllRead: (query: ArticleQuery) => invoke<number[]>("mark_all_read", { query }),
  imageDataUrl: (url: string) => invoke<string>("image_data_url", { url }),
  /** An article picture made small for the list (lib/thumbs.ts). */
  thumbnailDataUrl: (url: string) => invoke<string>("thumbnail_data_url", { url }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  addFeed: (url: string, folderId: number | null) => invoke<Added>("add_feed", { url, folderId }),
  getFeedInfo: (id: number) => invoke<FeedInfo>("get_feed_info", { id }),
  /** Checks the new URL (a page that names its feed works too) before it changes anything. */
  changeFeedUrl: (id: number, url: string) => invoke<FeedInfo>("change_feed_url", { id, url }),
  renameFeed: (id: number, title: string) => invoke<void>("rename_feed", { id, title }),
  moveFeed: (id: number, folderId: number | null) => invoke<void>("move_feed", { id, folderId }),
  deleteFeed: (id: number) => invoke<void>("delete_feed", { id }),
  /** 全文取得 on or off; articles already there are fetched when opened. */
  setFeedFullText: (id: number, on: boolean) => invoke<void>("set_feed_full_text", { id, on }),
  /** The article's page fetched now (`fullText` "pending"); the feed's text when that fails. */
  fetchFullText: (id: number) => invoke<Article>("fetch_full_text", { id }),
  /** The article as Markdown, dated in this machine's time zone. */
  articleMarkdown: (id: number) =>
    invoke<ArticleMarkdown>("article_markdown", { id, utcOffsetMinutes: utcOffsetMinutes() }),
  /** Asks stockroom nothing. */
  stockroomStatus: () => invoke<StockroomStatus>("stockroom_status"),
  /** Waits while the user answers stockroom's question showing `code` (up to 2 minutes). */
  stockroomPair: (code: string) => invoke<StockroomStatus>("stockroom_pair", { code }),
  stockroomDisconnect: () => invoke<StockroomStatus>("stockroom_disconnect"),
  /** stockroom's folders and tags, asked now. */
  stockroomChoices: () => invoke<StockroomChoices>("stockroom_choices"),
  setStockroomTarget: (folder: string | null, folderName: string | null, tags: string[]) =>
    invoke<StockroomStatus>("set_stockroom_target", { folder, folderName, tags }),
  /** `duplicate`: stockroom already had it. */
  sendToStockroom: (id: number) =>
    invoke<{ duplicate: boolean }>("send_to_stockroom", { id, utcOffsetMinutes: utcOffsetMinutes() }),
  /** An article's page fetched in the background after a refresh. */
  onFullText: (cb: (articleId: number) => void): Promise<UnlistenFn> =>
    listen<number>("full-text", (e) => cb(e.payload)),
  createFolder: (name: string) => invoke<number>("create_folder", { name }),
  renameFolder: (id: number, name: string) => invoke<void>("rename_folder", { id, name }),
  deleteFolder: (id: number) => invoke<void>("delete_folder", { id }),

  /** All feeds when `ids` is absent. Progress: `onRefreshProgress`. */
  refreshFeeds: (ids?: number[]) => invoke<RefreshSummary>("refresh_feeds", { ids: ids ?? null }),
  cancelRefresh: () => invoke<void>("cancel_refresh"),
  onRefreshProgress: (cb: (p: RefreshProgress) => void): Promise<UnlistenFn> =>
    listen<RefreshProgress>("refresh-progress", (e) => cb(e.payload)),

  /** The icons found so far; new ones: `onIconsChanged`. */
  feedIcons: () => invoke<FeedIcon[]>("feed_icons"),
  onIconsChanged: (cb: () => void): Promise<UnlistenFn> => listen("icons-changed", () => cb()),

  importOpml: (path: string) => invoke<Imported>("import_opml", { path }),
  extensionStatus: () => invoke<ExtensionStatus>("extension_status"),
  setExtensionReceiver: (enabled: boolean) => invoke<ExtensionStatus>("set_extension_receiver", { enabled }),
  /** A new key: every extension connected before is cut off. */
  resetExtensionToken: () => invoke<ExtensionStatus>("reset_extension_token"),
  /** Writes the extension out with its key and shows the folder; returns it. */
  installExtension: () => invoke<string>("install_extension"),
  answerExtensionPair: (id: string, allow: boolean) => invoke<void>("answer_extension_pair", { id, allow }),
  onExtensionPair: (cb: (r: ExtensionPairRequest) => void): Promise<UnlistenFn> =>
    listen<ExtensionPairRequest>("extension-pair", (e) => cb(e.payload)),
  onExtensionPairEnd: (cb: (id: string) => void): Promise<UnlistenFn> =>
    listen<string>("extension-pair-end", (e) => cb(e.payload)),
  /** A feed subscribed or a page saved from the browser: what to say, and what to show. */
  onExtensionChanged: (cb: (news: FromBrowser) => void): Promise<UnlistenFn> =>
    listen<FromBrowser>("extension-changed", (e) => cb(e.payload)),
  /** What the OS handed over since last asked (kept in Rust until the window takes it). */
  takeOpenRequests: () => invoke<OpenRequest[]>("take_open_requests"),
  onOpenRequested: (cb: () => void): Promise<UnlistenFn> => listen("open-requested", () => cb()),
  exportOpml: (path: string) => invoke<number>("export_opml", { path }),

  getAppSettings: () => invoke<AppSettings>("get_app_settings"),
  setAppSettings: (settings: AppSettings) => invoke<AppSettings>("set_app_settings", { settings }),
  revealLog: () => invoke<void>("reveal_log"),
  /** A version's release page on GitHub, in the browser (what changed). */
  openReleasePage: (version: string) => invoke<void>("open_release_page", { version }),
};

/** Whether 未読のみ applies (not to スター付き and あとで読む; db.rs `View::filters_unread`). */
export const filtersUnread = (v: View) => v.kind !== "starred" && v.kind !== "later";

export const sameView = (a: View, b: View) =>
  a.kind === b.kind && ("id" in a ? a.id : null) === ("id" in b ? b.id : null);
