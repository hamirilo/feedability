// Dev-only in-memory stand-in for the Rust backend, used when the UI is
// opened in a normal browser (`npm run dev`) instead of inside Tauri.
// Mirrors src-tauri/src/db.rs closely enough for UI work: the same filters,
// order and counts, a few feeds and folders, a refresh with progress.
import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type {
  Added,
  AppSettings,
  Article,
  ArticleQuery,
  ArticleRow,
  FeedIcon,
  FeedInfo,
  FeedRow,
  FolderRow,
  Imported,
  RefreshSummary,
  Sidebar,
} from "../lib/api";
import { filtersUnread } from "../lib/api";

type MockFeed = Omit<FeedRow, "unread"> & {
  addedAt?: number;
  /** The feed's own title, once the user has renamed it. */
  original?: string;
  /** Renamed by the user. */
  custom?: boolean;
};
type MockArticle = Omit<ArticleRow, "feedTitle"> & {
  html: string;
  author: string | null;
  /** When it went into あとで読む (db.rs `later_at`). */
  laterAt: number | null;
};

const folders: Omit<FolderRow, "unread">[] = [
  { id: 1, name: "技術" },
  { id: 2, name: "ニュース" },
];
const feeds: MockFeed[] = [
  { id: 1, title: "Rust Blog", url: "https://blog.rust-lang.org/feed.xml", siteUrl: "https://blog.rust-lang.org/", folderId: 1, error: null, fetchedAt: Date.now() - 600_000 },
  { id: 2, title: "Tauri", url: "https://tauri.app/blog/rss.xml", siteUrl: "https://tauri.app/", folderId: 1, error: null, fetchedAt: Date.now() - 600_000 },
  { id: 3, title: "世界のニュース", url: "https://news.example.jp/rss", siteUrl: "https://news.example.jp/", folderId: 2, error: null, fetchedAt: Date.now() - 600_000 },
  { id: 4, title: "止まったブログ", url: "https://gone.example.com/feed", siteUrl: null, folderId: null, error: "見つかりません（HTTP 404）\nhttps://gone.example.com/feed は 3 日前から応答がありません。サイトが移転した場合は、フィードの情報から新しい URL に変更できます。", fetchedAt: Date.now() - 3_600_000 },
  { id: 5, title: "個人の日記", url: "https://diary.example.com/atom.xml", siteUrl: "https://diary.example.com/", folderId: null, error: null, fetchedAt: Date.now() - 600_000 },
];
const articles: MockArticle[] = [];
/** Icons already drawn, by feed id (a feed's title is its first letter). */
const iconCache = new Map<number, string>();
let articleSeq = 0;
let feedSeq = feeds.length;
let folderSeq = folders.length;
const settings: AppSettings = { autoUpdate: true, refreshMinutes: 30, keepDays: 30, theme: "system", nextFeedAfterReadAll: false };

const LOREM =
  "フィードリーダーは、購読したサイトの新しい記事をまとめて読むための道具です。サイトを一つずつ見に行かなくても、更新があったものだけが届きます。";

function addArticle(feedId: number, published: number, n: number) {
  const id = ++articleSeq;
  const title = n % 7 === 0 ? `とても長いタイトルの記事 ${id}：一覧で 1 行に収まらないときに省略されることを確かめる` : `記事 ${id}：${["お知らせ", "リリース", "解説", "日記", "ニュース"][n % 5]}`;
  articles.push({
    id,
    feedId,
    title,
    summary: `${LOREM}（${id}）`,
    url: `https://example.com/articles/${id}`,
    published,
    read: n % 3 === 0,
    starred: n % 11 === 0,
    later: n % 13 === 0,
    laterAt: n % 13 === 0 ? published - n : null,
    author: n % 2 ? "山田 太郎" : null,
    html:
      `<p>${LOREM}</p><h2>見出し</h2><p>本文の中の <a href="https://example.com/">リンク</a> はブラウザで開きます。</p>` +
      `<ul><li>項目 1</li><li>項目 2</li></ul><blockquote>引用です。</blockquote>` +
      `<pre><code>fn main() {\n    println!("hello");\n}</code></pre>` +
      `<p><img src="https://example.com/photo-${id}.png" alt="写真"></p><p>${LOREM}</p>`,
  });
}

function seed() {
  const now = Date.now();
  for (let n = 0; n < 240; n++) addArticle((n % 5) + 1, now - n * 47 * 60_000, n);
}

const feedTitle = (id: number) => feeds.find((f) => f.id === id)?.title ?? "";

function matches(a: MockArticle, q: ArticleQuery): boolean {
  const v = q.view;
  if (v.kind === "starred" && !a.starred) return false;
  if (v.kind === "later" && a.laterAt === null) return false;
  if (v.kind === "feed" && a.feedId !== v.id) return false;
  if (v.kind === "folder" && feeds.find((f) => f.id === a.feedId)?.folderId !== v.id) return false;
  for (const word of q.search.toLowerCase().split(/\s+/).filter(Boolean)) {
    const hay = `${a.title}\n${a.summary}\n${feedTitle(a.feedId)}`.toLowerCase();
    if (!hay.includes(word)) return false;
  }
  return true;
}

function sidebar(): Sidebar {
  const unreadOf = (feedId: number) => articles.filter((a) => a.feedId === feedId && !a.read).length;
  const rows: FeedRow[] = feeds
    .map((f) => ({ ...f, unread: unreadOf(f.id) }))
    .sort((a, b) => a.title.localeCompare(b.title, "ja"));
  return {
    folders: folders
      .map((f) => ({ ...f, unread: rows.filter((x) => x.folderId === f.id).reduce((n, x) => n + x.unread, 0) }))
      .sort((a, b) => a.name.localeCompare(b.name, "ja")),
    feeds: rows,
    unread: rows.reduce((n, f) => n + f.unread, 0),
    starred: articles.filter((a) => a.starred).length,
    later: articles.filter((a) => a.laterAt !== null).length,
  };
}

/** A small PNG: a coloured rounded square with the feed's first letter, as a site's icon would be. */
function drawIcon(id: number, title: string): string {
  const cached = iconCache.get(id);
  if (cached) return cached;
  const size = 32;
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = `hsl(${(id * 67) % 360} 55% 48%)`;
  ctx.beginPath();
  ctx.roundRect(0, 0, size, size, 7);
  ctx.fill();
  ctx.fillStyle = "#fff";
  ctx.font = "bold 20px sans-serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(Array.from(title)[0] ?? "?", size / 2, size / 2 + 1);
  const url = canvas.toDataURL("image/png");
  iconCache.set(id, url);
  return url;
}

/** The icon pass finishing a moment after a subscription or a URL change. */
function iconsSoon() {
  setTimeout(() => void emit("icons-changed", null), 1500);
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function refresh(ids: number[] | null): Promise<RefreshSummary> {
  const targets = feeds.filter((f) => !ids || ids.includes(f.id));
  let added = 0;
  for (let i = 0; i < targets.length; i++) {
    await sleep(250);
    const f = targets[i];
    if (!f.error) {
      addArticle(f.id, Date.now(), articleSeq + 1);
      added++;
      f.fetchedAt = Date.now();
    }
    await emit("refresh-progress", { done: i + 1, total: targets.length });
  }
  const failed = targets.filter((f) => f.error).map((f) => ({ feedId: f.id, title: f.title, error: f.error! }));
  return { feeds: targets.length, newArticles: added, failed, cancelled: false };
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
async function handle(cmd: string, a: any): Promise<unknown> {
  switch (cmd) {
    case "get_app_settings":
      return { ...settings };
    case "set_app_settings":
      Object.assign(settings, a.settings);
      return { ...settings };
    case "get_sidebar":
      return sidebar();
    case "query_articles": {
      const q: ArticleQuery = a.query;
      const unreadOnly = q.unreadOnly && filtersUnread(q.view);
      return articles
        .filter((x) => matches(x, q) && (!unreadOnly || !x.read || x.id === q.keep))
        .sort((x, y) =>
          q.view.kind === "later"
            ? x.laterAt! - y.laterAt! || x.id - y.id
            : q.oldestFirst
              ? x.published - y.published || x.id - y.id
              : y.published - x.published || y.id - x.id,
        )
        .map(({ laterAt, ...x }): ArticleRow => ({
          ...x,
          later: laterAt !== null,
          feedTitle: feedTitle(x.feedId),
          summary: x.summary.slice(0, 160),
        }));
    }
    case "get_article": {
      const x = articles.find((y) => y.id === a.id);
      if (!x) throw "記事が見つかりません";
      const f = feeds.find((y) => y.id === x.feedId)!;
      const { laterAt, ...rest } = x;
      return { ...rest, later: laterAt !== null, feedTitle: f.title, siteUrl: f.siteUrl } satisfies Article;
    }
    case "set_read":
      for (const x of articles) if (a.ids.includes(x.id)) x.read = a.read;
      return null;
    case "set_starred": {
      const x = articles.find((y) => y.id === a.id);
      if (x) x.starred = a.starred;
      return null;
    }
    case "set_later": {
      const now = Date.now();
      for (const x of articles) if (a.ids.includes(x.id)) x.laterAt = a.later ? (x.laterAt ?? now) : null;
      return null;
    }
    case "mark_all_read": {
      const changed = articles.filter((x) => !x.read && matches(x, a.query));
      for (const x of changed) x.read = true;
      return changed.map((x) => x.id);
    }
    case "image_data_url": {
      // A grey picture with the image's name, in place of the real one.
      const name = String(a.url).split("/").pop();
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="480" height="270"><rect width="100%" height="100%" fill="#3a3b42"/><text x="50%" y="50%" fill="#9a9ba3" font-size="16" text-anchor="middle" font-family="sans-serif">${name}</text></svg>`;
      await sleep(300);
      return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
    }
    case "open_url":
      window.open(a.url, "_blank", "noopener");
      return null;
    case "add_feed": {
      const url = String(a.url).trim();
      if (!url) throw "URL を入力してください";
      if (feeds.some((f) => f.url === url)) throw "このフィードはすでに購読しています";
      await sleep(600);
      if (url.includes("404")) throw "見つかりません（HTTP 404）";
      const id = ++feedSeq;
      const title = url.replace(/^https?:\/\//, "").split("/")[0];
      feeds.push({ id, title, url, siteUrl: null, folderId: a.folderId, error: null, fetchedAt: Date.now(), addedAt: Date.now() });
      for (let n = 0; n < 5; n++) addArticle(id, Date.now() - n * 3_600_000, n + 1);
      iconsSoon();
      return { feedId: id, title, articles: 5 } satisfies Added;
    }
    case "get_feed_info": {
      const f = feeds.find((x) => x.id === a.id);
      if (!f) throw "フィードが見つかりません";
      const own = articles.filter((x) => x.feedId === f.id);
      return {
        id: f.id,
        title: f.title,
        originalTitle: f.original ?? f.title,
        customTitle: f.custom ? f.title : null,
        url: f.url,
        siteUrl: f.siteUrl,
        folderId: f.folderId,
        addedAt: f.addedAt ?? Date.now() - 40 * 86_400_000,
        fetchedAt: f.fetchedAt,
        error: f.error,
        articles: own.length,
        unread: own.filter((x) => !x.read).length,
      } satisfies FeedInfo;
    }
    case "change_feed_url": {
      const f = feeds.find((x) => x.id === a.id);
      if (!f) throw "フィードが見つかりません";
      const url = String(a.url).trim();
      if (!url) throw "URL を入力してください";
      if (feeds.some((x) => x.id !== f.id && x.url === url)) throw "このフィードはすでに購読しています";
      await sleep(800);
      if (url.includes("fail")) throw "フィードとして読めませんでした（モックの失敗）";
      f.url = url;
      f.error = null;
      f.fetchedAt = Date.now();
      iconCache.delete(f.id);
      iconsSoon();
      return handle("get_feed_info", { id: f.id });
    }
    case "feed_icons":
      // Most feeds have one; 個人の日記 has none, to show the fallback.
      return feeds
        .filter((f) => f.id !== 5)
        .map((f) => ({ feedId: f.id, icon: drawIcon(f.id, f.original ?? f.title) })) satisfies FeedIcon[];
    case "rename_feed": {
      const f = feeds.find((x) => x.id === a.id)!;
      f.original ??= f.title;
      const title = a.title.trim();
      f.title = title || f.original;
      f.custom = !!title;
      return null;
    }
    case "move_feed":
      feeds.find((x) => x.id === a.id)!.folderId = a.folderId;
      return null;
    case "delete_feed":
      feeds.splice(feeds.findIndex((x) => x.id === a.id), 1);
      for (let i = articles.length - 1; i >= 0; i--) if (articles[i].feedId === a.id) articles.splice(i, 1);
      return null;
    case "create_folder":
      if (!a.name.trim()) throw "フォルダの名前を入力してください";
      folders.push({ id: ++folderSeq, name: a.name.trim() });
      return folderSeq;
    case "rename_folder":
      if (!a.name.trim()) throw "フォルダの名前を入力してください";
      folders.find((x) => x.id === a.id)!.name = a.name.trim();
      return null;
    case "delete_folder":
      folders.splice(folders.findIndex((x) => x.id === a.id), 1);
      for (const f of feeds) if (f.folderId === a.id) f.folderId = null;
      return null;
    case "refresh_feeds":
      return refresh(a.ids);
    case "cancel_refresh":
      return null;
    case "import_opml":
      return { feedIds: [], skipped: 0 } satisfies Imported;
    case "export_opml":
      return feeds.length;
    case "reveal_log":
      throw "ブラウザ版ではログを表示できません";
    case "plugin:dialog|ask":
    case "plugin:dialog|confirm":
      return window.confirm(a.message);
    case "plugin:dialog|open":
    case "plugin:dialog|save":
      return null;
    default:
      console.warn("[mock] unhandled command", cmd, a);
      return null;
  }
}

export function installMockBackend() {
  seed();
  mockWindows("main");
  mockIPC((cmd, payload) => handle(cmd, payload), { shouldMockEvents: true });
  console.info("[mock] in-memory backend installed");
}
