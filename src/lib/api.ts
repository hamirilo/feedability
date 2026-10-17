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
}

export interface Sidebar {
  folders: FolderRow[];
  feeds: FeedRow[];
  unread: number;
  starred: number;
}

export interface ArticleQuery {
  view: View;
  unreadOnly: boolean;
  search: string;
  /** Listed even when read (the article open in the reader). */
  keep?: number | null;
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
}

export interface AppSettings {
  autoUpdate: boolean;
  /** 0 = only by hand. */
  refreshMinutes: number;
  /** 0 = keep read articles for good. */
  keepDays: number;
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
  /** Returns the ids it marked, for 元に戻す. */
  markAllRead: (query: ArticleQuery) => invoke<number[]>("mark_all_read", { query }),
  imageDataUrl: (url: string) => invoke<string>("image_data_url", { url }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),

  addFeed: (url: string, folderId: number | null) => invoke<Added>("add_feed", { url, folderId }),
  renameFeed: (id: number, title: string) => invoke<void>("rename_feed", { id, title }),
  moveFeed: (id: number, folderId: number | null) => invoke<void>("move_feed", { id, folderId }),
  deleteFeed: (id: number) => invoke<void>("delete_feed", { id }),
  createFolder: (name: string) => invoke<number>("create_folder", { name }),
  renameFolder: (id: number, name: string) => invoke<void>("rename_folder", { id, name }),
  deleteFolder: (id: number) => invoke<void>("delete_folder", { id }),

  /** All feeds when `ids` is absent. Progress: `onRefreshProgress`. */
  refreshFeeds: (ids?: number[]) => invoke<RefreshSummary>("refresh_feeds", { ids: ids ?? null }),
  cancelRefresh: () => invoke<void>("cancel_refresh"),
  onRefreshProgress: (cb: (p: RefreshProgress) => void): Promise<UnlistenFn> =>
    listen<RefreshProgress>("refresh-progress", (e) => cb(e.payload)),

  importOpml: (path: string) => invoke<Imported>("import_opml", { path }),
  exportOpml: (path: string) => invoke<number>("export_opml", { path }),

  getAppSettings: () => invoke<AppSettings>("get_app_settings"),
  setAppSettings: (settings: AppSettings) => invoke<AppSettings>("set_app_settings", { settings }),
  revealLog: () => invoke<void>("reveal_log"),
};

export const sameView = (a: View, b: View) =>
  a.kind === b.kind && ("id" in a ? a.id : null) === ("id" in b ? b.id : null);
