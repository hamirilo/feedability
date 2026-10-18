/**
 * The one store (zustand): what is shown (view, filters, the list, the open
 * article), the dialogs and the toasts. The list is the query's result as
 * is; after a change in Rust it is asked again, not patched up here, except
 * for read / starred on rows already shown (so reading with 未読のみ on
 * doesn't pull the row out from under the user).
 */
import { create } from "zustand";
import {
  api,
  sameView,
  type AppSettings,
  type Article,
  type ArticleQuery,
  type ArticleRow,
  type FeedRow,
  type RefreshProgress,
  type Sidebar,
  type View,
  type ExtensionPairRequest,
} from "./lib/api";
import { PANE_DEFAULT, savedWidth, type Pane } from "./lib/layout";
import { DEFAULT_LIST_LAYOUT, validLayout, type ListLayout } from "./lib/listLayout";
import { warn } from "./lib/log";
import {
  DEFAULT_LINE_HEIGHT,
  DEFAULT_TEXT_SIZE,
  DEFAULT_TEXT_WIDTH,
  LINE_HEIGHTS,
  TEXT_SIZES,
  TEXT_WIDTHS,
  validStep,
} from "./lib/reading";
import { applyTheme } from "./lib/theme";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export type ToastKind = "success" | "info" | "error";

export interface ToastOptions {
  title: string;
  detail?: string;
  kind?: ToastKind;
  action?: ToastAction;
  /** How long it stays, in ms. */
  duration?: number;
  /** Rewrite this toast in place (a result following its "…しています"), if it is still showing. */
  replace?: number;
}

export interface Toast extends Required<Pick<ToastOptions, "title" | "kind" | "duration">> {
  id: number;
  /** Bumped when the toast is rewritten in place; restarts its countdown. */
  rev: number;
  detail?: string;
  action?: ToastAction;
  leaving?: boolean;
}

/** The settings' pages (SettingsDialog.tsx); ⌘, opens the last one seen. */
export type SettingsPageId = "general" | "reading" | "extension" | "stockroom";
const SETTINGS_PAGES: SettingsPageId[] = ["general", "reading", "extension", "stockroom"];

export type Dialog =
  | { kind: "settings"; page: SettingsPageId }
  | { kind: "shortcuts" }
  /** `url`: filled in (a `feed:` link the OS handed over). */
  | { kind: "addFeed"; url?: string }
  | {
      kind: "prompt";
      title: string;
      label: string;
      value: string;
      okLabel: string;
      onOk: (value: string) => Promise<void> | void;
    }
  | { kind: "moveFeed"; feedId: number }
  | { kind: "feedInfo"; feedId: number }
  /**
   * stockroom's connection question; `thenSend`: the article to send once
   * connected; `fromSettings`: back to the settings when it closes.
   */
  | { kind: "stockroomPair"; thenSend?: number; fromSettings?: boolean };

export interface Updating {
  done: number;
  total: number | null;
  installing?: boolean;
}

/** Display preferences: the same in every session (localStorage). */
const PREFS_KEY = "feedability.prefs";
interface Prefs {
  unreadOnly: boolean;
  /** Oldest first (not in あとで読む, which keeps the order it was filled in). */
  oldestFirst: boolean;
  /** The list's rows (docs/LIST_LAYOUT.md). */
  listLayout: ListLayout;
  collapsed: number[];
  /** Pane widths in px (docs/LAYOUT.md). */
  sidebarWidth: number;
  listWidth: number;
  /** Reader display: indexes into the steps of lib/reading.ts (docs/READING.md). */
  textSize: number;
  lineHeight: number;
  textWidth: number;
  /** The settings' page seen last (desktop-platform UI §9.1). */
  settingsPage: SettingsPageId;
}
function loadPrefs(): Prefs {
  const fallback: Prefs = {
    unreadOnly: true,
    oldestFirst: false,
    listLayout: DEFAULT_LIST_LAYOUT,
    collapsed: [],
    sidebarWidth: PANE_DEFAULT.sidebar,
    listWidth: PANE_DEFAULT.list,
    textSize: DEFAULT_TEXT_SIZE,
    lineHeight: DEFAULT_LINE_HEIGHT,
    textWidth: DEFAULT_TEXT_WIDTH,
    settingsPage: "general",
  };
  try {
    const p = { ...fallback, ...JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}") };
    return {
      ...p,
      sidebarWidth: savedWidth("sidebar", p.sidebarWidth),
      listWidth: savedWidth("list", p.listWidth),
      listLayout: validLayout(p.listLayout),
      textSize: validStep(p.textSize, TEXT_SIZES.length, fallback.textSize),
      lineHeight: validStep(p.lineHeight, LINE_HEIGHTS.length, fallback.lineHeight),
      textWidth: validStep(p.textWidth, TEXT_WIDTHS.length, fallback.textWidth),
      settingsPage: SETTINGS_PAGES.includes(p.settingsPage) ? p.settingsPage : fallback.settingsPage,
    };
  } catch {
    return fallback;
  }
}
function savePrefs(p: Prefs) {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(p));
  } catch {
    /* private mode: the defaults next time */
  }
}

export interface State {
  sidebar: Sidebar;
  view: View;
  unreadOnly: boolean;
  oldestFirst: boolean;
  listLayout: ListLayout;
  /** The saved pane widths; what is drawn is kept inside the window (usePaneWidths). */
  sidebarWidth: number;
  listWidth: number;
  /** Reader display: indexes into TEXT_SIZES / LINE_HEIGHTS / TEXT_WIDTHS. */
  textSize: number;
  lineHeight: number;
  textWidth: number;
  search: string;
  articles: ArticleRow[];
  /** The list has been asked at least once (empty states wait for it). */
  loaded: boolean;
  selectedId: number | null;
  article: Article | null;
  /** Folders closed in the sidebar. */
  collapsed: Set<number>;
  settings: AppSettings;
  /** Feed icons (data: URLs) by feed id; not part of the sidebar, which is asked again after every read. */
  icons: Record<number, string>;
  refreshing: RefreshProgress | null;
  updating: Updating | null;
  dialog: Dialog | null;
  /** A browser extension asks to connect (over everything, docs/EXTENSION.md). */
  extensionPair: ExtensionPairRequest | null;
  toasts: Toast[];

  init: () => Promise<void>;
  reloadSidebar: () => Promise<void>;
  reloadIcons: () => Promise<void>;
  reload: () => Promise<void>;
  setView: (v: View) => void;
  /** The feeds in the order the sidebar shows them: each folder's, then the loose ones. */
  feedOrder: () => FeedRow[];
  /** Opens a feed; `selectFirstUnread`: and its first unread article once the list is in. */
  openFeed: (id: number, selectFirstUnread?: boolean) => Promise<void>;
  /** The next feed with unread articles after the open feed or folder (wrapping), or null. */
  nextUnreadFeed: () => number | null;
  /** N: the next unread article; at the end of the list, the next feed with unread ones. */
  nextUnread: () => Promise<void>;
  /** Shift+J / Shift+K: the next / previous feed. */
  moveFeed: (delta: 1 | -1) => void;
  setUnreadOnly: (on: boolean) => void;
  setOldestFirst: (on: boolean) => void;
  setListLayout: (layout: ListLayout) => void;
  /** While dragging a pane's handle: in memory only; `savePaneWidths` on release. */
  setPaneWidth: (pane: Pane, width: number) => void;
  savePaneWidths: () => void;
  /** Double click on a handle: the default width, saved. */
  resetPaneWidth: (pane: Pane) => void;
  setTextSize: (i: number) => void;
  setLineHeight: (i: number) => void;
  setTextWidth: (i: number) => void;
  /** 標準に戻す: size, line height and width. */
  resetReading: () => void;
  setSearch: (s: string) => void;
  toggleFolder: (id: number) => void;
  select: (id: number | null) => Promise<void>;
  /** Next / previous article in the list (J / K). */
  move: (delta: 1 | -1) => void;
  toggleRead: () => Promise<void>;
  toggleStar: () => Promise<void>;
  /** Into or out of あとで読む (L). */
  toggleLater: () => Promise<void>;
  /** 全文取得 on or off for a feed (docs/FULL_TEXT.md). */
  toggleFullText: (feedId: number) => Promise<void>;
  /** The open article again when its page has arrived (`full-text`, or fetched on opening). */
  loadFullText: (id: number) => Promise<void>;
  markAllRead: () => Promise<void>;
  /** The unread articles above / below the selected one. */
  markAbove: () => Promise<void>;
  markBelow: () => Promise<void>;
  /**
   * Refreshes `ids` (all feeds when absent). `quiet`: the automatic refresh,
   * which tells of no failures (the sidebar marks the feed; a dead feed
   * would otherwise bring the same red toast every half hour).
   */
  refresh: (ids?: number[], quiet?: boolean) => Promise<void>;
  setSettings: (s: AppSettings) => Promise<void>;
  setRefreshing: (p: RefreshProgress | null) => void;
  setUpdating: (u: Updating | null) => void;
  /** The settings page seen last. */
  settingsPage: SettingsPageId;
  /** ⌘, and the menu: the page seen last; a page: that one (and remembered). */
  openSettings: (page?: SettingsPageId) => void;
  openDialog: (d: Dialog) => void;
  closeDialog: () => void;
  setExtensionPair: (r: ExtensionPairRequest | null) => void;

  toast: (message: string, error?: boolean, action?: ToastAction) => void;
  /** A toast with a title, detail line and kind. Returns its id. */
  notify: (t: ToastOptions) => number;
  dismissToast: (id: number) => void;
}

const EMPTY_SIDEBAR: Sidebar = { folders: [], feeds: [], unread: 0, starred: 0, later: 0 };
const prefs = loadPrefs();
let toastSeq = 0;
let searchTimer: ReturnType<typeof setTimeout> | undefined;
/** The latest list query: an older answer arriving late is dropped. */
let querySeq = 0;
let articleSeq = 0;
/** 開いて 2 秒後 (markReadOn "delay"): the pending mark-as-read of the open article. */
let readTimer: ReturnType<typeof setTimeout> | undefined;
const READ_DELAY_MS = 2000;
const cancelReadTimer = () => {
  clearTimeout(readTimer);
  readTimer = undefined;
};
/**
 * The open article, when it was in あとで読む as it was opened: leaving it
 * (another article, another view) means it has been read, and it goes out
 * of the queue (docs/LATER.md). Not when the user put it in while reading,
 * or pressed L on it: then they said what they want.
 */
let finishOnLeave: number | null = null;

export const useStore = create<State>((set, get) => {
  const query = (): ArticleQuery => {
    const s = get();
    return { view: s.view, unreadOnly: s.unreadOnly, search: s.search.trim(), keep: s.selectedId, oldestFirst: s.oldestFirst };
  };
  const savePrefsNow = () =>
    savePrefs({
      unreadOnly: get().unreadOnly,
      oldestFirst: get().oldestFirst,
      listLayout: get().listLayout,
      collapsed: [...get().collapsed],
      sidebarWidth: get().sidebarWidth,
      listWidth: get().listWidth,
      textSize: get().textSize,
      lineHeight: get().lineHeight,
      textWidth: get().textWidth,
      settingsPage: get().settingsPage,
    });
  /** Asks the list again; whether the answer was used (false: a newer query overtook it, or it failed). */
  const load = async (): Promise<boolean> => {
    const seq = ++querySeq;
    try {
      const articles = await api.queryArticles(query());
      if (seq !== querySeq) return false;
      set({ articles, loaded: true });
      return true;
    } catch (e) {
      if (seq === querySeq) fail(e);
      return false;
    }
  };
  /** Switches the view; resolves when its list is in. */
  const switchView = (v: View): Promise<boolean> => {
    if (sameView(v, get().view)) return Promise.resolve(false);
    leave(null);
    cancelReadTimer();
    set({ view: v, selectedId: null, article: null });
    return load();
  };
  /** Read on rows already shown (Rust has marked them): sidebar, then the toast with 元に戻す. */
  const announceRead = async (ids: number[]) => {
    patch(ids, { read: true });
    await get().reloadSidebar();
    get().notify({
      title: `${ids.length} 件を既読にしました`,
      kind: "success",
      action: {
        label: "元に戻す",
        onClick: () => {
          api.setRead(ids, false).then(() => {
            patch(ids, { read: false });
            void get().reloadSidebar();
            void get().reload();
          }, fail);
        },
      },
    });
  };
  const markRead = async (ids: number[]) => {
    if (!ids.length) return;
    try {
      await api.setRead(ids, true);
      await announceRead(ids);
    } catch (e) {
      fail(e);
    }
  };
  /** Leaving the open article for `next`: a finished あとで読む goes out of it. Its row stays until the list is asked again. */
  const leave = (next: number | null) => {
    const id = finishOnLeave;
    if (id === null || id === next) return;
    finishOnLeave = null;
    api.setLater([id], false).then(() => {
      patch([id], { later: false });
      void get().reloadSidebar();
    }, fail);
  };
  const fail = (e: unknown) => get().toast(String(e), true);
  /** Read / starred on the rows already shown and on the open article. */
  const patch = (ids: number[], change: Partial<Pick<ArticleRow, "read" | "starred" | "later">>) => {
    const set_ = new Set(ids);
    const a = get().article;
    set({
      articles: get().articles.map((r) => (set_.has(r.id) ? { ...r, ...change } : r)),
      article: a && set_.has(a.id) ? { ...a, ...change } : a,
    });
  };

  return {
    sidebar: EMPTY_SIDEBAR,
    view: { kind: "all" },
    unreadOnly: prefs.unreadOnly,
    oldestFirst: prefs.oldestFirst,
    listLayout: prefs.listLayout,
    sidebarWidth: prefs.sidebarWidth,
    listWidth: prefs.listWidth,
    textSize: prefs.textSize,
    lineHeight: prefs.lineHeight,
    textWidth: prefs.textWidth,
    settingsPage: prefs.settingsPage,
    search: "",
    articles: [],
    loaded: false,
    selectedId: null,
    article: null,
    collapsed: new Set(prefs.collapsed),
    settings: { autoUpdate: true, refreshMinutes: 30, keepDays: 30, theme: "system", nextFeedAfterReadAll: false, markReadOn: "open" },
    icons: {},
    refreshing: null,
    updating: null,
    dialog: null,
    extensionPair: null,
    toasts: [],

    init: async () => {
      try {
        const settings = await api.getAppSettings();
        set({ settings });
        applyTheme(settings.theme);
      } catch (e) {
        warn("settings", e);
      }
      await Promise.all([get().reloadSidebar(), get().reload(), get().reloadIcons()]);
    },

    reloadIcons: async () => {
      try {
        const icons: Record<number, string> = {};
        for (const i of await api.feedIcons()) icons[i.feedId] = i.icon;
        set({ icons });
      } catch (e) {
        // Icons are a nicety: the generic one stays.
        warn("icons", e);
      }
    },

    reloadSidebar: async () => {
      try {
        const sidebar = await api.getSidebar();
        // The open feed or folder is gone (deleted): back to すべて.
        const v = get().view;
        const gone =
          (v.kind === "feed" && !sidebar.feeds.some((f) => f.id === v.id)) ||
          (v.kind === "folder" && !sidebar.folders.some((f) => f.id === v.id));
        set({ sidebar });
        if (gone) get().setView({ kind: "all" });
      } catch (e) {
        fail(e);
      }
    },

    reload: async () => {
      await load();
    },

    setView: (v) => void switchView(v),

    feedOrder: () => {
      const { folders, feeds } = get().sidebar;
      return [...folders.flatMap((d) => feeds.filter((f) => f.folderId === d.id)), ...feeds.filter((f) => f.folderId === null)];
    },

    openFeed: async (id, selectFirstUnread = false) => {
      const feed = get().sidebar.feeds.find((f) => f.id === id);
      // A feed inside a closed folder opens the folder, so it shows in the sidebar.
      if (feed?.folderId != null && get().collapsed.has(feed.folderId)) {
        const collapsed = new Set(get().collapsed);
        collapsed.delete(feed.folderId);
        set({ collapsed });
        savePrefsNow();
      }
      const v: View = { kind: "feed", id };
      const same = sameView(v, get().view);
      const used = same ? (selectFirstUnread ? await load() : false) : await switchView(v);
      if (!selectFirstUnread || !used || !sameView(v, get().view)) return;
      const first = get().articles.find((a) => !a.read);
      if (first) await get().select(first.id);
    },

    nextUnreadFeed: () => {
      const { view } = get();
      const order = get().feedOrder();
      let pos = -1;
      if (view.kind === "feed") pos = order.findIndex((f) => f.id === view.id);
      else if (view.kind === "folder") {
        for (let i = order.length - 1; i >= 0; i--) if (order[i].folderId === view.id) { pos = i; break; }
      }
      for (let k = 1; k <= order.length; k++) {
        const f = order[(pos + k) % order.length];
        if (f.unread > 0) return f.id;
      }
      return null;
    },

    nextUnread: async () => {
      const { articles, selectedId, view } = get();
      const i = articles.findIndex((a) => a.id === selectedId);
      const next = articles.slice(i + 1).find((a) => !a.read);
      if (next) return void (await get().select(next.id));
      if (view.kind === "feed" || view.kind === "folder") {
        const id = get().nextUnreadFeed();
        if (id !== null) return void (await get().openFeed(id, true));
      }
      get().toast("未読の記事はもうありません");
    },

    moveFeed: (delta) => {
      const { view, unreadOnly } = get();
      const order = get().feedOrder();
      if (!order.length) return;
      const pos = view.kind === "feed" ? order.findIndex((f) => f.id === view.id) : delta > 0 ? -1 : order.length;
      for (let k = 1; k <= order.length; k++) {
        const f = order[(((pos + delta * k) % order.length) + order.length) % order.length];
        if (f.id === (view.kind === "feed" ? view.id : null)) return;
        if (unreadOnly && f.unread === 0) continue;
        void get().openFeed(f.id);
        return;
      }
    },

    setUnreadOnly: (on) => {
      set({ unreadOnly: on });
      savePrefsNow();
      void get().reload();
    },

    setOldestFirst: (on) => {
      set({ oldestFirst: on });
      savePrefsNow();
      void get().reload();
    },

    // Display only: the list isn't asked again.
    setListLayout: (layout) => {
      set({ listLayout: validLayout(layout) });
      savePrefsNow();
    },

    setPaneWidth: (pane, width) => set(pane === "sidebar" ? { sidebarWidth: width } : { listWidth: width }),
    savePaneWidths: () => savePrefsNow(),
    resetPaneWidth: (pane) => {
      get().setPaneWidth(pane, PANE_DEFAULT[pane]);
      savePrefsNow();
    },

    setTextSize: (i) => {
      set({ textSize: validStep(i, TEXT_SIZES.length, get().textSize) });
      savePrefsNow();
    },
    setLineHeight: (i) => {
      set({ lineHeight: validStep(i, LINE_HEIGHTS.length, get().lineHeight) });
      savePrefsNow();
    },
    setTextWidth: (i) => {
      set({ textWidth: validStep(i, TEXT_WIDTHS.length, get().textWidth) });
      savePrefsNow();
    },
    resetReading: () => {
      set({ textSize: DEFAULT_TEXT_SIZE, lineHeight: DEFAULT_LINE_HEIGHT, textWidth: DEFAULT_TEXT_WIDTH });
      savePrefsNow();
    },

    setSearch: (search) => {
      set({ search });
      clearTimeout(searchTimer);
      searchTimer = setTimeout(() => void get().reload(), 150);
    },

    toggleFolder: (id) => {
      const collapsed = new Set(get().collapsed);
      if (!collapsed.delete(id)) collapsed.add(id);
      set({ collapsed });
      savePrefsNow();
    },

    select: async (id) => {
      const seq = ++articleSeq;
      cancelReadTimer();
      leave(id);
      set({ selectedId: id, article: id === get().article?.id ? get().article : null });
      if (id === null) return;
      try {
        const article = await api.getArticle(id);
        if (seq !== articleSeq) return;
        set({ article });
        if (article.fullText === "pending") void get().loadFullText(id);
        if (finishOnLeave !== id) finishOnLeave = article.later ? id : null;
        if (article.read) return;
        const { markReadOn } = get().settings;
        if (markReadOn === "open") {
          await api.setRead([id], true);
          patch([id], { read: true });
          void get().reloadSidebar();
        } else if (markReadOn === "delay") {
          // Only if it is still the open article then (J / K past it cancels).
          readTimer = setTimeout(() => {
            readTimer = undefined;
            if (get().selectedId !== id) return;
            api.setRead([id], true).then(() => {
              patch([id], { read: true });
              void get().reloadSidebar();
            }, fail);
          }, READ_DELAY_MS);
        }
      } catch (e) {
        if (seq === articleSeq) fail(e);
      }
    },

    move: (delta) => {
      const { articles, selectedId } = get();
      if (!articles.length) return;
      const i = articles.findIndex((a) => a.id === selectedId);
      const next = i < 0 ? (delta > 0 ? 0 : articles.length - 1) : i + delta;
      if (next < 0 || next >= articles.length) return;
      void get().select(articles[next].id);
    },

    toggleRead: async () => {
      const a = get().article;
      if (!a) return;
      cancelReadTimer();
      try {
        await api.setRead([a.id], !a.read);
        patch([a.id], { read: !a.read });
        void get().reloadSidebar();
      } catch (e) {
        fail(e);
      }
    },

    toggleStar: async () => {
      const a = get().article;
      if (!a) return;
      try {
        await api.setStarred(a.id, !a.starred);
        patch([a.id], { starred: !a.starred });
        void get().reloadSidebar();
      } catch (e) {
        fail(e);
      }
    },

    toggleLater: async () => {
      const a = get().article;
      if (!a) return;
      finishOnLeave = null;
      try {
        await api.setLater([a.id], !a.later);
        patch([a.id], { later: !a.later });
        void get().reloadSidebar();
      } catch (e) {
        fail(e);
      }
    },

    toggleFullText: async (feedId) => {
      const on = !get().sidebar.feeds.find((f) => f.id === feedId)?.fullText;
      try {
        await api.setFeedFullText(feedId, on);
        await get().reloadSidebar();
        get().toast(on ? "全文取得をオンにしました。記事のページから本文を取得します" : "全文取得をオフにしました");
        // The open article of that feed follows at once.
        const a = get().article;
        if (a?.feedId === feedId) {
          const next = await api.getArticle(a.id);
          if (get().article?.id !== a.id) return;
          set({ article: { ...get().article!, html: next.html, fullText: next.fullText } });
          if (next.fullText === "pending") void get().loadFullText(a.id);
        }
      } catch (e) {
        fail(e);
      }
    },

    loadFullText: async (id) => {
      try {
        const current = get().article;
        if (current?.id !== id || current.fullText === "page") return;
        // Asked while the article was open: fetched now. From the pass: there already.
        const next = current.fullText === "pending" ? await api.fetchFullText(id) : await api.getArticle(id);
        const a = get().article;
        if (a?.id !== id) return;
        // Only the text: read, star and あとで読む may have changed meanwhile.
        set({ article: { ...a, html: next.html, fullText: next.fullText } });
      } catch (e) {
        // The feed's text stays; a page that can't be had is no error to show.
        warn("full text", e);
      }
    },

    markAllRead: async () => {
      try {
        const ids = await api.markAllRead(query());
        if (!ids.length) return;
        await announceRead(ids);
        const { view, settings } = get();
        if (settings.nextFeedAfterReadAll && (view.kind === "feed" || view.kind === "folder")) {
          const id = get().nextUnreadFeed();
          if (id !== null) await get().openFeed(id);
        }
      } catch (e) {
        fail(e);
      }
    },

    markAbove: async () => {
      const { articles, selectedId } = get();
      const i = articles.findIndex((a) => a.id === selectedId);
      if (i < 0) return;
      await markRead(articles.slice(0, i).filter((a) => !a.read).map((a) => a.id));
    },

    markBelow: async () => {
      const { articles, selectedId } = get();
      const i = articles.findIndex((a) => a.id === selectedId);
      if (i < 0) return;
      await markRead(articles.slice(i + 1).filter((a) => !a.read).map((a) => a.id));
    },

    refresh: async (ids, quiet = false) => {
      if (get().refreshing) return;
      if (!get().sidebar.feeds.length && !ids) return;
      set({ refreshing: { done: 0, total: ids?.length ?? get().sidebar.feeds.length } });
      try {
        const r = await api.refreshFeeds(ids);
        if (r.failed.length && !quiet) {
          get().notify({
            title: `${r.failed.length} 件のフィードを更新できませんでした`,
            detail: r.failed
              .slice(0, 3)
              .map((f) => `${f.title}：${f.error}`)
              .join("\n"),
            kind: "error",
            duration: 8000,
          });
        } else if (r.cancelled) {
          get().toast("更新を中止しました");
        }
      } catch (e) {
        // A refresh already running (the timer and ⌘R meeting) isn't worth a toast.
        if (String(e) !== "更新中です" && !quiet) fail(e);
      } finally {
        set({ refreshing: null });
        await Promise.all([get().reloadSidebar(), get().reload()]);
      }
    },

    setSettings: async (s) => {
      try {
        const settings = await api.setAppSettings(s);
        set({ settings });
        applyTheme(settings.theme);
      } catch (e) {
        fail(e);
      }
    },

    setRefreshing: (refreshing) => {
      // Progress events after the refresh returned would bring the bar back.
      if (get().refreshing) set({ refreshing });
    },
    setUpdating: (updating) => set({ updating }),
    openSettings: (page) => {
      const p = page ?? get().settingsPage;
      set({ dialog: { kind: "settings", page: p }, settingsPage: p });
      savePrefsNow();
    },
    openDialog: (dialog) => set({ dialog }),
    closeDialog: () => set({ dialog: null }),
    setExtensionPair: (extensionPair) => set({ extensionPair }),

    toast: (message, error, action) => void get().notify({ title: message, kind: error ? "error" : "info", action }),
    notify: ({ title, detail, kind = "info", action, duration, replace }) => {
      const rev = ++toastSeq;
      const fields = { title, detail, kind, action, duration: duration ?? (action ? 8000 : 4000), rev };
      if (replace !== undefined && get().toasts.some((x) => x.id === replace && !x.leaving)) {
        set({ toasts: get().toasts.map((x) => (x.id === replace ? { ...fields, id: replace } : x)) });
        return replace;
      }
      const t: Toast = { ...fields, id: rev };
      // At most three at once: the oldest go first.
      const live = get().toasts.filter((x) => !x.leaving);
      for (const old of live.slice(0, Math.max(0, live.length - 2))) get().dismissToast(old.id);
      set({ toasts: [...get().toasts, t] });
      return t.id;
    },
    dismissToast: (id) => {
      if (!get().toasts.some((t) => t.id === id && !t.leaving)) return;
      set({ toasts: get().toasts.map((t) => (t.id === id ? { ...t, leaving: true } : t)) });
      setTimeout(() => set({ toasts: get().toasts.filter((t) => t.id !== id) }), 200);
    },
  };
});

/** The title of what the list shows. */
export function viewTitle(s: Pick<State, "view" | "sidebar">): string {
  const v = s.view;
  switch (v.kind) {
    case "all":
      return "すべての記事";
    case "starred":
      return "スター付き";
    case "later":
      return "あとで読む";
    case "feed":
      return s.sidebar.feeds.find((f) => f.id === v.id)?.title ?? "";
    case "folder":
      return s.sidebar.folders.find((f) => f.id === v.id)?.name ?? "";
  }
}
