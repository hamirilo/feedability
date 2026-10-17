/**
 * The articles of the open view, newest first. Rows have one height so the
 * list can be virtual (thousands of articles stay smooth). The controls above
 * it are the same in every view and keep their places (docs/UI.md): what
 * can't be used now is greyed out, not removed.
 */
import { useVirtualizer } from "@tanstack/react-virtual";
import { CheckCheck, Clock, Search, Star, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { filtersUnread } from "../lib/api";
import { runCommand } from "../lib/commands";
import { listDate } from "../lib/format";
import { composing } from "../lib/ime";
import { withKey } from "../lib/shortcuts";
import { dragRegion } from "../lib/titleBar";
import { useStore, viewTitle } from "../store";

const ROW = 92;

function Segment() {
  const unreadOnly = useStore((s) => s.unreadOnly);
  const setUnreadOnly = useStore((s) => s.setUnreadOnly);
  // スター付き and あとで読む list what was kept, read or not: greyed out
  // there rather than hidden, so the search field doesn't move.
  const applies = useStore((s) => filtersUnread(s.view));
  const seg = (on: boolean) =>
    `px-2.5 py-0.5 rounded ${unreadOnly === on ? "bg-raised text-fg shadow" : "text-dim enabled:hover:text-fg"}`;
  return (
    <div
      className={`flex shrink-0 rounded-md bg-bg p-0.5 text-xs ${applies ? "" : "opacity-40"}`}
      title={applies ? withKey("未読のみ表示の切り替え", "list.unreadOnly") : "スター付きとあとで読むでは、読んだ記事も表示します"}
    >
      <button className={seg(true)} onClick={() => setUnreadOnly(true)} disabled={!applies}>
        未読
      </button>
      <button className={seg(false)} onClick={() => setUnreadOnly(false)} disabled={!applies}>
        すべて
      </button>
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
  const canMarkAll = useStore((s) => s.articles.some((a) => !a.read));
  const scroller = useRef<HTMLDivElement>(null);
  const virtual = useVirtualizer({
    count: articles.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW,
    overscan: 8,
  });

  // The selected row stays in sight when moving with J / K.
  useEffect(() => {
    const i = articles.findIndex((a) => a.id === selectedId);
    if (i >= 0) virtual.scrollToIndex(i, { align: "auto" });
  }, [selectedId, articles, virtual]);

  // Another view or filter starts at the top.
  const view = useStore((s) => s.view);
  const unreadOnly = useStore((s) => s.unreadOnly);
  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 });
  }, [view, unreadOnly]);

  return (
    <section className="flex h-full w-[380px] shrink-0 flex-col border-r border-line">
      <header {...dragRegion(true)} className="shrink-0 border-b border-line px-3" style={{ paddingTop: "max(calc(var(--titlebar) - 30px), 8px)" }}>
        <div className="flex h-7 items-center gap-2">
          <h1 className="min-w-0 flex-1 truncate text-[14px] font-semibold">{title}</h1>
          <span className="shrink-0 text-xs tabular-nums text-dim">{articles.length} 件</span>
        </div>
        <div className="flex items-center gap-2 py-2">
          <Segment />
          <SearchField />
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
                className={`absolute inset-x-0 cursor-default border-b border-line/60 px-3 py-2 ${
                  selected ? "bg-accent/25" : "hover:bg-hover/50"
                }`}
                style={{ height: ROW, transform: `translateY(${v.start}px)` }}
              >
                <div className="flex items-center gap-1.5 text-[11px] text-dim">
                  <span
                    className={`h-1.5 w-1.5 shrink-0 rounded-full ${a.read ? "bg-transparent" : "bg-accent"}`}
                    aria-label={a.read ? undefined : "未読"}
                  />
                  <span className="min-w-0 flex-1 truncate">{a.feedTitle}</span>
                  {a.later && <Clock size={11} className="shrink-0 text-accent" aria-label="あとで読む" />}
                  {a.starred && <Star size={11} className="shrink-0 fill-current text-star" />}
                  <span className="shrink-0 tabular-nums">{listDate(a.published)}</span>
                </div>
                <div className={`mt-0.5 truncate ${a.read ? "text-fg/70" : "font-semibold"}`}>{a.title}</div>
                <div className="mt-0.5 line-clamp-2 text-xs leading-snug text-dim">{a.summary}</div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
