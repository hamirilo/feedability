/**
 * Every operation of the app, defined once. The menu bar (menuBar.ts), the
 * right-click menus, the key handling (App.tsx) and the shortcut list (?)
 * are all made from this table, so a name and its key read the same
 * everywhere.
 *
 * Keys are written like "Mod+Shift+R": Mod is ⌘ on macOS and Ctrl elsewhere.
 * The first key is the one menus show.
 */
import {
  CheckCheck,
  Clock,
  CircleDot,
  ExternalLink,
  FolderPlus,
  Plus,
  RefreshCw,
  Settings,
  Star,
  type LucideIcon,
} from "lucide-react";
import {
  addFeedDialog,
  deleteFeed,
  deleteFolder,
  exportOpml,
  importOpml,
  moveFeedDialog,
  newFolderDialog,
  openInBrowser,
  renameFeedDialog,
  renameFolderDialog,
} from "./actions";
import { api, filtersUnread } from "./api";
import { LIST_LAYOUTS } from "./listLayout";
import { DEFAULT_TEXT_SIZE, TEXT_SIZES } from "./reading";
import { checkForUpdate } from "./update";
import { useStore, type State } from "../store";

type S = State;

export interface Command {
  id: string;
  /** Name in the shortcut list and the default label. */
  title: string;
  /** The label for the current state ("スターを付ける" / "スターを外す"). */
  label?: (s: S) => string;
  icon?: LucideIcon;
  keys?: string[];
  enabled?: (s: S) => boolean;
  checked?: (s: S) => boolean;
  run: (s: S) => void;
  /**
   * Who handles the key: "list" (the window's key handler, not while typing
   * in a field; the default), "global" (also while typing, e.g. ⌘F) or
   * "menu" (only the menu bar's accelerator).
   */
  keyBy?: "list" | "global" | "menu";
}

const hasArticle = (s: S) => s.article !== null;
const openFeed = (s: S) => (s.view.kind === "feed" ? s.view.id : null);
const openFolder = (s: S) => (s.view.kind === "folder" ? s.view.id : null);
const hasFeeds = (s: S) => s.sidebar.feeds.length > 0;
/** The open feed has 全文取得 on (docs/FULL_TEXT.md). */
const fullTextOn = (s: S) => !!s.sidebar.feeds.find((f) => f.id === openFeed(s))?.fullText;

/** Whether an article is selected and the list has an unread row above (-1) or below (1) it. */
function unreadOn(s: S, side: -1 | 1): boolean {
  const i = s.articles.findIndex((a) => a.id === s.selectedId);
  if (i < 0) return false;
  return (side < 0 ? s.articles.slice(0, i) : s.articles.slice(i + 1)).some((a) => !a.read);
}

/** Space: a screen further down the article; at its end, the next article. */
function pageDown(s: S) {
  const el = document.getElementById("reader-scroll");
  if (!el || el.scrollTop + el.clientHeight >= el.scrollHeight - 4) return s.move(1);
  el.scrollBy({ top: el.clientHeight * 0.85, behavior: "smooth" });
}

export const COMMANDS: Command[] = [
  // ---------------------------------------------------------------- app
  { id: "app.settings", title: "設定…", icon: Settings, keys: ["Mod+,"], keyBy: "global", run: (s) => s.openDialog({ kind: "settings" }) },
  { id: "app.checkUpdate", title: "アップデートを確認…", run: () => void checkForUpdate(true) },
  { id: "help.shortcuts", title: "キーボードショートカット", keys: ["?"], run: (s) => s.openDialog({ kind: "shortcuts" }) },
  { id: "help.log", title: "ログを表示", run: () => void api.revealLog().catch((e) => useStore.getState().toast(String(e), true)) },

  // -------------------------------------------------------------- feeds
  { id: "feed.add", title: "フィードを追加…", icon: Plus, keys: ["Mod+N"], keyBy: "global", run: () => addFeedDialog() },
  { id: "folder.new", title: "フォルダを作成…", icon: FolderPlus, keys: ["Mod+Shift+N"], keyBy: "global", run: () => newFolderDialog() },
  {
    id: "feed.refreshAll",
    title: "すべて更新",
    icon: RefreshCw,
    keys: ["Mod+R"],
    keyBy: "global",
    enabled: (s) => hasFeeds(s) && !s.refreshing,
    run: (s) => void s.refresh(),
  },
  {
    id: "feed.refresh",
    title: "このフィードを更新",
    keys: ["Mod+Shift+R"],
    keyBy: "global",
    enabled: (s) => openFeed(s) !== null && !s.refreshing,
    run: (s) => {
      const id = openFeed(s);
      if (id !== null) void s.refresh([id]);
    },
  },
  {
    id: "feed.info",
    title: "フィードの情報…",
    keys: ["Mod+I"],
    keyBy: "global",
    enabled: (s) => openFeed(s) !== null,
    run: (s) => {
      const id = openFeed(s);
      if (id !== null) s.openDialog({ kind: "feedInfo", feedId: id });
    },
  },
  {
    id: "feed.rename",
    title: "フィードの名前を変更…",
    enabled: (s) => openFeed(s) !== null,
    run: (s) => {
      const id = openFeed(s);
      if (id !== null) renameFeedDialog(id);
    },
  },
  {
    id: "feed.move",
    title: "フォルダへ移動…",
    enabled: (s) => openFeed(s) !== null,
    run: (s) => {
      const id = openFeed(s);
      if (id !== null) moveFeedDialog(id);
    },
  },
  {
    id: "feed.fullText",
    title: "全文を取得する",
    label: (s) => (fullTextOn(s) ? "全文の取得をやめる" : "全文を取得する"),
    enabled: (s) => openFeed(s) !== null,
    checked: fullTextOn,
    run: (s) => {
      const id = openFeed(s);
      if (id !== null) void s.toggleFullText(id);
    },
  },
  {
    id: "feed.delete",
    title: "購読をやめる…",
    enabled: (s) => openFeed(s) !== null,
    run: (s) => {
      const id = openFeed(s);
      if (id !== null) void deleteFeed(id);
    },
  },
  {
    id: "folder.rename",
    title: "フォルダの名前を変更…",
    enabled: (s) => openFolder(s) !== null,
    run: (s) => {
      const id = openFolder(s);
      if (id !== null) renameFolderDialog(id);
    },
  },
  {
    id: "folder.delete",
    title: "フォルダを削除…",
    enabled: (s) => openFolder(s) !== null,
    run: (s) => {
      const id = openFolder(s);
      if (id !== null) void deleteFolder(id);
    },
  },
  {
    id: "folder.refresh",
    title: "フォルダ内を更新",
    enabled: (s) => openFolder(s) !== null && !s.refreshing,
    run: (s) => {
      const id = openFolder(s);
      const ids = s.sidebar.feeds.filter((f) => f.folderId === id).map((f) => f.id);
      if (ids.length) void s.refresh(ids);
    },
  },
  { id: "opml.import", title: "OPML を読み込む…", run: () => void importOpml() },
  { id: "opml.export", title: "OPML に書き出す…", enabled: hasFeeds, run: () => void exportOpml() },

  // ---------------------------------------------------------------- list
  { id: "go.next", title: "次の記事", keys: ["J", "ArrowDown"], run: (s) => s.move(1) },
  { id: "go.prev", title: "前の記事", keys: ["K", "ArrowUp"], run: (s) => s.move(-1) },
  { id: "go.nextUnread", title: "次の未読の記事", keys: ["N"], run: (s) => void s.nextUnread() },
  { id: "go.nextFeed", title: "次のフィード", keys: ["Shift+J"], enabled: hasFeeds, run: (s) => s.moveFeed(1) },
  { id: "go.prevFeed", title: "前のフィード", keys: ["Shift+K"], enabled: hasFeeds, run: (s) => s.moveFeed(-1) },
  {
    id: "list.unreadOnly",
    title: "未読のみ表示",
    keys: ["Mod+U"],
    keyBy: "global",
    enabled: (s) => filtersUnread(s.view),
    checked: (s) => s.unreadOnly,
    run: (s) => s.setUnreadOnly(!s.unreadOnly),
  },
  {
    id: "list.oldestFirst",
    title: "古い順に並べる",
    enabled: (s) => s.view.kind !== "later",
    checked: (s) => s.oldestFirst,
    run: (s) => s.setOldestFirst(!s.oldestFirst),
  },
  // 表示 › 一覧のレイアウト (docs/LIST_LAYOUT.md): list.layout.standard / .compact / .thumbs.
  ...LIST_LAYOUTS.map(
    (l): Command => ({
      id: `list.layout.${l.id}`,
      title: `一覧のレイアウト：${l.label}`,
      label: () => l.label,
      checked: (s) => s.listLayout === l.id,
      run: (s) => s.setListLayout(l.id),
    }),
  ),
  {
    id: "list.markAllRead",
    title: "一覧をすべて既読にする",
    icon: CheckCheck,
    keys: ["Mod+Shift+A"],
    keyBy: "global",
    enabled: (s) => s.articles.some((a) => !a.read),
    run: (s) => void s.markAllRead(),
  },
  {
    id: "list.markAboveRead",
    title: "ここより上を既読にする",
    enabled: (s) => unreadOn(s, -1),
    run: (s) => void s.markAbove(),
  },
  {
    id: "list.markBelowRead",
    title: "ここより下を既読にする",
    enabled: (s) => unreadOn(s, 1),
    run: (s) => void s.markBelow(),
  },
  {
    id: "edit.search",
    title: "検索",
    keys: ["Mod+F"],
    keyBy: "global",
    run: () => document.querySelector<HTMLInputElement>("#search")?.select(),
  },
  { id: "view.all", title: "すべての記事", keys: ["Mod+1"], keyBy: "global", run: (s) => s.setView({ kind: "all" }) },
  { id: "view.later", title: "あとで読む", keys: ["Mod+2"], keyBy: "global", run: (s) => s.setView({ kind: "later" }) },
  { id: "view.starred", title: "スター付き", keys: ["Mod+3"], keyBy: "global", run: (s) => s.setView({ kind: "starred" }) },

  {
    id: "view.textBigger",
    title: "文字を大きく",
    keys: ["Mod+Plus"],
    keyBy: "global",
    enabled: (s) => s.textSize < TEXT_SIZES.length - 1,
    run: (s) => s.setTextSize(s.textSize + 1),
  },
  {
    id: "view.textSmaller",
    title: "文字を小さく",
    keys: ["Mod+Minus"],
    keyBy: "global",
    enabled: (s) => s.textSize > 0,
    run: (s) => s.setTextSize(s.textSize - 1),
  },
  {
    id: "view.textReset",
    title: "文字の大きさを標準に",
    keys: ["Mod+0"],
    keyBy: "global",
    enabled: (s) => s.textSize !== DEFAULT_TEXT_SIZE,
    run: (s) => s.setTextSize(DEFAULT_TEXT_SIZE),
  },

  // ------------------------------------------------------------- article
  {
    id: "article.read",
    title: "既読 / 未読",
    label: (s) => (s.article && !s.article.read ? "既読にする" : "未読にする"),
    icon: CircleDot,
    keys: ["M"],
    enabled: hasArticle,
    run: (s) => void s.toggleRead(),
  },
  {
    id: "article.star",
    title: "スター",
    label: (s) => (s.article?.starred ? "スターを外す" : "スターを付ける"),
    icon: Star,
    keys: ["S"],
    enabled: hasArticle,
    checked: (s) => !!s.article?.starred,
    run: (s) => void s.toggleStar(),
  },
  {
    id: "article.later",
    title: "あとで読む",
    label: (s) => (s.article?.later ? "あとで読むから外す" : "あとで読む"),
    icon: Clock,
    keys: ["L"],
    enabled: hasArticle,
    checked: (s) => !!s.article?.later,
    run: (s) => void s.toggleLater(),
  },
  {
    id: "article.page",
    title: "本文をスクロール（最後まで読んだら次の記事）",
    keys: ["Space"],
    enabled: hasArticle,
    run: (s) => pageDown(s),
  },
  {
    id: "article.open",
    title: "ブラウザで開く",
    icon: ExternalLink,
    keys: ["V", "Enter"],
    enabled: (s) => !!s.article?.url,
    run: (s) => s.article?.url && void openInBrowser(s.article.url),
  },
];

const byId = new Map(COMMANDS.map((c) => [c.id, c]));

export function command(id: string): Command {
  const c = byId.get(id);
  if (!c) throw new Error(`unknown command ${id}`);
  return c;
}

export const isEnabled = (c: Command, s: S) => c.enabled?.(s) ?? true;
export const labelOf = (c: Command, s: S) => c.label?.(s) ?? c.title;

/**
 * The last command run and from where. A key can reach both the menu bar's
 * accelerator and the page's handler (which one depends on the platform),
 * so the second of the two within a moment is dropped.
 */
let lastRun = { id: "", by: "", at: 0 };

function runFrom(c: Command, s: S, by: "key" | "menu") {
  const now = performance.now();
  if (lastRun.id === c.id && lastRun.by !== by && now - lastRun.at < 250) return;
  lastRun = { id: c.id, by, at: now };
  c.run(s);
}

/** Runs a command by id if it is enabled now (menus, buttons). */
export function runCommand(id: string) {
  const c = command(id);
  const s = useStore.getState();
  if (isEnabled(c, s)) runFrom(c, s, "menu");
}

// ---------------------------------------------------------------- keys

/** The key part of a combo matched against an event (layout-independent where it can be). */
function keyMatches(key: string, e: KeyboardEvent): boolean {
  if (/^[A-Z]$/.test(key)) return e.code === `Key${key}`;
  if (/^[0-9]$/.test(key)) return e.code === `Digit${key}` || e.code === `Numpad${key}`;
  switch (key) {
    case "?":
      return e.key === "?";
    case "Plus":
      // ⌘= (US) and ⌘+ / ⌘; with Shift (JIS) alike.
      return e.key === "+" || e.key === "=" || e.code === "Equal" || e.code === "NumpadAdd";
    case "Minus":
      return e.key === "-" || e.code === "Minus" || e.code === "NumpadSubtract";
    case ",":
      return e.code === "Comma";
    case "Space":
      return e.key === " ";
    default:
      return e.key === key;
  }
}

/** Whether `e` is exactly `combo` (no extra modifiers; Shift is free for "?" and "Plus"). */
export function comboMatches(combo: string, e: KeyboardEvent): boolean {
  const parts = combo.split("+");
  const key = parts.pop()!;
  const mods = new Set(parts);
  if ((e.metaKey || e.ctrlKey) !== mods.has("Mod")) return false;
  if (e.altKey !== mods.has("Alt")) return false;
  if (key !== "?" && key !== "Plus" && e.shiftKey !== mods.has("Shift")) return false;
  return keyMatches(key, e);
}

/**
 * The window's key handler: runs the command bound to `e`, if any. `typing`:
 * the focus is in a text field, where only "global" keys count (single
 * letters must still type). Returns whether it took the key.
 */
export function runKey(e: KeyboardEvent, typing: boolean): boolean {
  const s = useStore.getState();
  for (const c of COMMANDS) {
    const by = c.keyBy ?? "list";
    if (by === "menu" || (typing && by !== "global")) continue;
    if (!c.keys?.some((k) => comboMatches(k, e))) continue;
    e.preventDefault();
    if (isEnabled(c, s)) runFrom(c, s, "key");
    return true;
  }
  return false;
}
