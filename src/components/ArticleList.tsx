/**
 * The articles of the open view, newest first (or oldest, 古い順). Rows have
 * one height per layout (標準 / コンパクト / 画像付き, docs/LIST_LAYOUT.md) so
 * the list can be virtual (thousands of articles stay smooth). The controls
 * above it are the same in every view and layout and keep their places
 * (docs/UI.md): what can't be used now is greyed out, not removed.
 */
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowDownWideNarrow, ArrowUpNarrowWide, CheckCheck, Clock, LayoutList, List, Rows3, Search, Star, X, type LucideIcon } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { filtersUnread, type ArticleRow } from "../lib/api";
import { runCommand } from "../lib/commands";
import { listDate } from "../lib/format";
import { composing } from "../lib/ime";
import { LIST_LAYOUTS, rowHeight, type ListLayout } from "../lib/listLayout";
import { cachedThumb, wantThumb } from "../lib/thumbs";
import { withKey } from "../lib/shortcuts";
import { dragRegion } from "../lib/titleBar";
import { useStore, viewTitle } from "../store";
import { commandItem, ContextMenu, type MenuState } from "./ContextMenu";
import { FeedIcon } from "./FeedIcon";
import { ResizeHandle, usePaneWidths } from "./ResizeHandle";

const LAYOUT_ICONS: Record<ListLayout, LucideIcon> = { standard: Rows3, compact: List, thumbs: LayoutList };

/** 標準 / コンパクト / 画像付き, at the end of the title row: the row below keeps its controls where they were. */
function LayoutSegment() {
  const layout = useStore((s) => s.listLayout);
  const setLayout = useStore((s) => s.setListLayout);
  return (
    <div className="flex shrink-0 rounded-md bg-bg p-0.5" role="radiogroup" aria-label="一覧のレイアウト">
      {LIST_LAYOUTS.map((l) => {
        const Icon = LAYOUT_ICONS[l.id];
        const on = layout === l.id;
        return (
          <button
            key={l.id}
            role="radio"
            aria-checked={on}
            aria-label={l.label}
            title={l.label}
            onClick={() => setLayout(l.id)}
            className={`rounded px-1.5 py-0.5 ${on ? "bg-raised text-fg shadow" : "text-dim hover:text-fg"}`}
          >
            <Icon size={13} />
          </button>
        );
      })}
    </div>
  );
}

/** A row's picture in 画像付き: asked for while the row is drawn, a grey square until (or unless) it comes. */
function Thumb({ url }: { url: string }) {
  const [data, setData] = useState(() => cachedThumb(url));
  useEffect(() => {
    const known = cachedThumb(url);
    setData(known);
    if (known !== undefined) return;
    return wantThumb(url, setData);
  }, [url]);
  return (
    <div className="h-16 w-16 shrink-0 overflow-hidden rounded-md bg-hover/70">
      {data && <img src={data} alt="" draggable={false} className="h-full w-full object-cover" />}
    </div>
  );
}

/** The dot, the icon and name of the feed, the marks and the date. */
function MetaLine({ a }: { a: ArticleRow }) {
  return (
    <div className="flex items-center gap-1.5 text-[11px] text-dim">
      <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${a.read ? "bg-transparent" : "bg-accent"}`} aria-label={a.read ? undefined : "未読"} />
      <FeedIcon feedId={a.feedId} size={12} />
      <span className="min-w-0 flex-1 truncate">{a.feedTitle}</span>
      {a.later && <Clock size={11} className="shrink-0 text-accent" aria-label="あとで読む" />}
      {a.starred && <Star size={11} className="shrink-0 fill-current text-star" />}
      <span className="shrink-0 tabular-nums">{listDate(a.published)}</span>
    </div>
  );
}

function RowBody({ a, layout }: { a: ArticleRow; layout: ListLayout }) {
  if (layout === "compact") {
    // The title only: the feed is its icon (and the tooltip).
    return (
      <div className="flex h-full items-center gap-1.5" title={a.feedTitle}>
        <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${a.read ? "bg-transparent" : "bg-accent"}`} aria-label={a.read ? undefined : "未読"} />
        <FeedIcon feedId={a.feedId} size={12} />
        <span className={`min-w-0 flex-1 truncate ${a.read ? "text-fg/70" : "font-semibold"}`}>{a.title}</span>
        {a.later && <Clock size={11} className="shrink-0 text-accent" aria-label="あとで読む" />}
        {a.starred && <Star size={11} className="shrink-0 fill-current text-star" />}
        <span className="shrink-0 text-[11px] tabular-nums text-dim">{listDate(a.published)}</span>
      </div>
    );
  }
  const text = (
    <div className="min-w-0 flex-1">
      <MetaLine a={a} />
      <div className={`mt-0.5 truncate ${a.read ? "text-fg/70" : "font-semibold"}`}>{a.title}</div>
      <div className="mt-0.5 line-clamp-2 text-xs leading-snug text-dim">{a.summary}</div>
    </div>
  );
  if (layout === "thumbs" && a.thumbUrl) {
    return (
      <div className="flex h-full items-center gap-2.5">
        <Thumb url={a.thumbUrl} />
        {text}
      </div>
    );
  }
  return text;
}

/** The view's unread count from the sidebar (no extra query); null where 未読のみ doesn't apply. */
function useUnreadCount(): number | null {
  return useStore((s) => {
    const v = s.view;
    if (v.kind === "all") return s.sidebar.unread;
    if (v.kind === "feed") return s.sidebar.feeds.find((f) => f.id === v.id)?.unread ?? 0;
    if (v.kind === "folder") return s.sidebar.folders.find((f) => f.id === v.id)?.unread ?? 0;
    return null;
  });
}

/**
 * 未読 / すべて: tabs along the header's bottom edge, which list is shown, apart
 * from the tools (search, order, すべて既読) above them.
 */
function UnreadTabs() {
  const unreadOnly = useStore((s) => s.unreadOnly);
  const setUnreadOnly = useStore((s) => s.setUnreadOnly);
  // スター付き and あとで読む list what was kept, read or not: greyed out
  // there rather than hidden, so nothing below moves.
  const applies = useStore((s) => filtersUnread(s.view));
  const unread = useUnreadCount();
  const tab = (on: boolean, label: string, count?: number | null) => {
    const active = applies && unreadOnly === on;
    return (
      <button
        role="tab"
        aria-selected={active}
        onClick={() => setUnreadOnly(on)}
        disabled={!applies}
        className={`-mb-px flex items-center gap-1.5 border-b-2 px-1 pt-1 pb-1.5 ${
          active ? "border-accent font-semibold text-fg" : "border-transparent text-dim enabled:hover:text-fg"
        }`}
      >
        {label}
        {count != null && <span className="text-[11px] font-normal tabular-nums text-dim">{count}</span>}
      </button>
    );
  };
  return (
    <div
      role="tablist"
      aria-label="表示する記事"
      className={`flex gap-4 text-[13px] ${applies ? "" : "opacity-40"}`}
      title={applies ? withKey("未読のみ表示の切り替え", "list.unreadOnly") : "スター付きとあとで読むでは、読んだ記事も表示します"}
    >
      {tab(true, "未読", unread)}
      {tab(false, "すべて")}
    </div>
  );
}

function SearchField() {
  const search = useStore((s) => s.search);
  const setSearch = useStore((s) => s.setSearch);
  return (
    <div className="relative min-w-0 flex-1">
      <Search size={13} className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-dim" />
      <input
        id="search"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && !composing(e)) {
            setSearch("");
            e.currentTarget.blur();
          }
          // Down into the list.
          if (e.key === "ArrowDown" && !composing(e)) {
            e.preventDefault();
            e.currentTarget.blur();
            runCommand("go.next");
          }
        }}
        placeholder="検索"
        spellCheck={false}
        className="w-full rounded-md border border-line bg-bg py-1 pr-6 pl-7 text-[13px] outline-none focus:border-accent"
      />
      {/* Hidden, not removed, when empty: the field keeps its width. */}
      <button
        onClick={() => setSearch("")}
        aria-label="検索をクリア"
        className="absolute top-1/2 right-1 -translate-y-1/2 rounded p-0.5 text-dim hover:text-fg"
        style={{ visibility: search ? "visible" : "hidden" }}
      >
        <X size={13} />
      </button>
    </div>
  );
}

function OrderButton() {
  const oldestFirst = useStore((s) => s.oldestFirst);
  // あとで読む keeps the order it was filled in: greyed out there, not hidden.
  const applies = useStore((s) => s.view.kind !== "later");
  const Icon = oldestFirst ? ArrowUpNarrowWide : ArrowDownWideNarrow;
  const tip = oldestFirst ? "古い順（クリックで新しい順に）" : "新しい順（クリックで古い順に）";
  return (
    <button
      onClick={() => runCommand("list.oldestFirst")}
      disabled={!applies}
      title={applies ? withKey(tip, "list.oldestFirst") : "あとで読むは入れた順に並びます"}
      aria-label="並び順の切り替え"
      className="shrink-0 rounded-md p-1.5 text-dim hover:bg-hover hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent"
    >
      <Icon size={16} />
    </button>
  );
}

function Empty() {
  const hasFeeds = useStore((s) => s.sidebar.feeds.length > 0);
  const unreadOnly = useStore((s) => s.unreadOnly);
  const search = useStore((s) => s.search);
  const view = useStore((s) => s.view);
  if (!hasFeeds) {
    return (
      <div className="flex flex-col items-center gap-3 px-6 pt-24 text-center">
        <p className="text-[15px] font-semibold">フィードを購読しましょう</p>
        <p className="text-xs leading-relaxed text-dim">
          ブログやニュースサイトの URL を入れると、そのサイトのフィードを探して購読します。
        </p>
        <div className="mt-2 flex gap-2">
          <button onClick={() => runCommand("feed.add")} className="rounded-md bg-accent px-3 py-1.5 font-medium text-white hover:brightness-110">
            フィードを追加
          </button>
          <button onClick={() => runCommand("opml.import")} className="rounded-md border border-line px-3 py-1.5 hover:bg-hover">
            OPML を読み込む
          </button>
        </div>
      </div>
    );
  }
  return (
    <p className="px-6 pt-24 text-center text-dim">
      {search.trim()
        ? "見つかりませんでした"
        : view.kind === "later"
          ? "あとで読む記事はありません。記事を開いて L で入れられます"
          : view.kind === "starred"
            ? "スター付きの記事はありません"
            : unreadOnly
              ? "未読の記事はありません"
              : "記事はありません"}
    </p>
  );
}

export function ArticleList() {
  const articles = useStore((s) => s.articles);
  const loaded = useStore((s) => s.loaded);
  const selectedId = useStore((s) => s.selectedId);
  const select = useStore((s) => s.select);
  const title = useStore(viewTitle);
  const layout = useStore((s) => s.listLayout);
  const row = rowHeight(layout);
  const canMarkAll = useStore((s) => s.articles.some((a) => !a.read));
  const width = usePaneWidths().list;
  const scroller = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<MenuState>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const virtual = useVirtualizer({
    count: articles.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => row,
    overscan: 8,
  });

  // Another layout: the rows have another height (the selected one is brought back into sight below).
  useEffect(() => {
    virtual.measure();
  }, [row, virtual]);

  // The selected row stays in sight when moving with J / K.
  useEffect(() => {
    const i = articles.findIndex((a) => a.id === selectedId);
    if (i >= 0) virtual.scrollToIndex(i, { align: "auto" });
  }, [selectedId, articles, virtual, row]);

  // Another view, filter or order starts at the top.
  const view = useStore((s) => s.view);
  const unreadOnly = useStore((s) => s.unreadOnly);
  const oldestFirst = useStore((s) => s.oldestFirst);
  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 });
  }, [view, unreadOnly, oldestFirst]);

  // Right-click selects the row first: the menu's commands act on the open
  // article, the same as in the menu bar (so it waits for the article).
  const rowMenu = (e: React.MouseEvent, id: number) => {
    e.preventDefault();
    const { clientX: x, clientY: y } = e;
    void select(id).then(() =>
      setMenu({
        x,
        y,
        items: [
          commandItem("article.read"),
          commandItem("article.star"),
          commandItem("article.later"),
          commandItem("article.open"),
          "sep",
          commandItem("list.markAboveRead"),
          commandItem("list.markBelowRead"),
        ],
      }),
    );
  };

  return (
    <section className="relative flex h-full shrink-0 flex-col border-r border-line" style={{ width }}>
      <header {...dragRegion(true)} className="shrink-0 border-b border-line px-3" style={{ paddingTop: "max(calc(var(--titlebar) - 30px), 8px)" }}>
        <div className="flex h-7 items-center gap-2">
          <h1 className="min-w-0 flex-1 truncate text-[14px] font-semibold">{title}</h1>
          <span className="shrink-0 text-xs tabular-nums text-dim">{articles.length} 件</span>
          <LayoutSegment />
        </div>
        <div className="flex items-center gap-2 pt-2 pb-1.5">
          <SearchField />
          <OrderButton />
          <button
            onClick={() => runCommand("list.markAllRead")}
            disabled={!canMarkAll}
            title={withKey(canMarkAll ? "一覧をすべて既読にする" : "未読の記事がありません", "list.markAllRead")}
            aria-label="一覧をすべて既読にする"
            className="shrink-0 rounded-md p-1.5 text-dim hover:bg-hover hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent"
          >
            <CheckCheck size={16} />
          </button>
        </div>
        <UnreadTabs />
      </header>
      <div ref={scroller} className="min-h-0 flex-1 overflow-y-auto" role="listbox" aria-label="記事">
        {loaded && articles.length === 0 && <Empty />}
        <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
          {virtual.getVirtualItems().map((v) => {
            const a = articles[v.index];
            const selected = a.id === selectedId;
            return (
              <div
                key={a.id}
                role="option"
                aria-selected={selected}
                onClick={() => void select(a.id)}
                onContextMenu={(e) => rowMenu(e, a.id)}
                className={`absolute inset-x-0 cursor-default border-b border-line/60 px-3 ${layout === "compact" ? "" : "py-2"} ${
                  selected ? "bg-accent/25" : "hover:bg-hover/50"
                }`}
                style={{ height: row, transform: `translateY(${v.start}px)` }}
              >
                <RowBody a={a} layout={layout} />
              </div>
            );
          })}
        </div>
      </div>
      <ContextMenu menu={menu} onClose={closeMenu} />
      <ResizeHandle pane="list" />
    </section>
  );
}
