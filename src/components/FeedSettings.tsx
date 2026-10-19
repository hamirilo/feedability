/**
 * 設定 › フィード (docs/SETTINGS.md): every feed in one list, to change many
 * at once — 全文取得, a folder, refresh, unsubscribe — and to add or bring
 * them in and out (OPML). The bar of actions stays put and is greyed out
 * while nothing is picked (UI Standard §10).
 */
import { AlertCircle, Download, Plus, RefreshCw, Search, Trash2, Upload } from "lucide-react";
import { useMemo, useRef, useState } from "react";
import { deleteFeeds, exportOpml, importOpml, moveFeeds, setFullTextMany } from "../lib/actions";
import type { FeedRow } from "../lib/api";
import { useStore } from "../store";
import { FeedIcon } from "./FeedIcon";
import { RowButton, Section, Switch } from "./SettingsFrame";

const select =
  "h-7 rounded-md border border-line bg-raised px-2 text-xs outline-none focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/40 disabled:opacity-50";

export function FeedSettings() {
  const feeds = useStore((s) => s.sidebar.feeds);
  const folders = useStore((s) => s.sidebar.folders);
  const refreshing = useStore((s) => s.refreshing);
  const openDialog = useStore((s) => s.openDialog);
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);
  /** Where a ⇧-click picks from. */
  const anchor = useRef<number | null>(null);

  const folderName = (id: number | null) => (id === null ? null : (folders.find((f) => f.id === id)?.name ?? null));
  // The sidebar's order: by folder, then the feeds outside the folders.
  const ordered = useMemo(() => {
    const byFolder = folders.flatMap((d) => feeds.filter((f) => f.folderId === d.id));
    return [...byFolder, ...feeds.filter((f) => f.folderId === null)];
  }, [feeds, folders]);
  const q = query.trim().toLowerCase();
  const shown = q
    ? ordered.filter((f) => [f.title, f.url, folderName(f.folderId) ?? ""].some((t) => t.toLowerCase().includes(q)))
    : ordered;
  // Only what is still there and in sight counts as picked.
  const ids = shown.filter((f) => picked.has(f.id)).map((f) => f.id);
  const all = shown.length > 0 && ids.length === shown.length;

  const pick = (f: FeedRow, range: boolean) => {
    const next = new Set(picked);
    const a = shown.findIndex((x) => x.id === anchor.current);
    const b = shown.findIndex((x) => x.id === f.id);
    if (range && a >= 0) {
      for (const x of shown.slice(Math.min(a, b), Math.max(a, b) + 1)) next.add(x.id);
    } else {
      if (next.has(f.id)) next.delete(f.id);
      else next.add(f.id);
      anchor.current = f.id;
    }
    setPicked(next);
  };
  const pickAll = () => setPicked(all ? new Set() : new Set(shown.map((f) => f.id)));

  const run = async (job: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await job();
    } finally {
      setBusy(false);
    }
  };
  const none = ids.length === 0 || busy;
  const fullTextOn = ids.length > 0 && ids.every((id) => feeds.find((f) => f.id === id)?.fullText);

  return (
    <>
      <Section title="購読">
        <div className="flex flex-wrap items-center gap-2 px-3 py-2.5">
          <RowButton primary onClick={() => openDialog({ kind: "addFeed", fromSettings: true })}>
            <Plus size={13} />
            フィードを追加
          </RowButton>
          <span className="flex-1" />
          <RowButton onClick={() => void importOpml()}>
            <Download size={13} />
            OPML を読み込む
          </RowButton>
          <RowButton onClick={() => void exportOpml()} disabled={feeds.length === 0}>
            <Upload size={13} />
            OPML に書き出す
          </RowButton>
        </div>
      </Section>

      <Section title={`フィード（${feeds.length} 件）`} sub="チェックを付けたフィードをまとめて変えます。⇧ を押しながらクリックすると、間のフィードもまとめて選びます">
        {/* The actions on the picked feeds: always here, greyed out while none is picked. */}
        <div className="sticky top-0 z-10 flex flex-wrap items-center justify-end gap-1.5 rounded-t-lg bg-panel px-3 py-2">
          <label className="mr-1 flex items-center gap-1.5 text-xs" title="選んだフィードの全文取得">
            全文取得
            <Switch
              checked={fullTextOn}
              disabled={none}
              onChange={(on) => void run(() => setFullTextMany(ids, on))}
            />
          </label>
          <select
            className={select}
            aria-label="選んだフィードをフォルダへ移動"
            disabled={none}
            value=""
            onChange={(e) => {
              const v = e.target.value;
              if (v) void run(() => moveFeeds(ids, v === "none" ? null : Number(v)));
            }}
          >
            <option value="">フォルダへ移動…</option>
            <option value="none">（フォルダなし）</option>
            {folders.map((f) => (
              <option key={f.id} value={f.id}>
                {f.name}
              </option>
            ))}
          </select>
          <RowButton disabled={none || !!refreshing} onClick={() => void run(() => useStore.getState().refresh(ids))}>
            <RefreshCw size={13} />
            更新
          </RowButton>
          <button
            type="button"
            disabled={none}
            onClick={() =>
              void run(async () => {
                if (await deleteFeeds(ids)) setPicked(new Set());
              })
            }
            className="flex h-7 items-center gap-1.5 rounded-md border border-danger/50 px-2.5 text-xs whitespace-nowrap text-danger outline-none hover:bg-danger/10 focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-50"
          >
            <Trash2 size={13} />
            購読をやめる
          </button>
        </div>
        <div className="flex items-center gap-2 px-3 py-2">
          <input
            type="checkbox"
            aria-label="すべて選択"
            className="size-4 shrink-0 accent-[var(--color-accent)]"
            checked={all}
            ref={(el) => {
              if (el) el.indeterminate = ids.length > 0 && !all;
            }}
            disabled={shown.length === 0}
            onChange={pickAll}
          />
          <label className="relative min-w-0 flex-1">
            <Search size={13} className="pointer-events-none absolute top-1/2 left-2 -translate-y-1/2 text-dim" />
            <input
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="名前・URL・フォルダで絞り込む"
              spellCheck={false}
              autoComplete="off"
              className="h-7 w-full rounded-md border border-line bg-raised pr-2 pl-7 text-xs outline-none focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/40"
            />
          </label>
          <span className="shrink-0 text-xs text-dim tabular-nums">{ids.length ? `${ids.length} 件を選択中` : "選択なし"}</span>
          <span className="w-14 shrink-0 text-right text-[11px] text-dim">全文取得</span>
        </div>
        {shown.length === 0 && (
          <p className="px-3 py-4 text-center text-xs text-dim">
            {feeds.length === 0 ? "まだ購読していません。「フィードを追加」か OPML から追加します" : "一致するフィードはありません"}
          </p>
        )}
        {shown.map((f) => (
          <FeedLine key={f.id} feed={f} folder={folderName(f.folderId)} picked={picked.has(f.id)} onPick={(range) => pick(f, range)} />
        ))}
      </Section>
    </>
  );
}

function FeedLine({
  feed,
  folder,
  picked,
  onPick,
}: {
  feed: FeedRow;
  folder: string | null;
  picked: boolean;
  onPick: (range: boolean) => void;
}) {
  return (
    <div
      className={`flex cursor-default items-center gap-2 px-3 py-1.5 ${picked ? "bg-accent/15" : "hover:bg-hover"}`}
      onClick={(e) => onPick(e.shiftKey)}
    >
      <input
        type="checkbox"
        aria-label={`「${feed.title}」を選ぶ`}
        className="size-4 shrink-0 accent-[var(--color-accent)]"
        checked={picked}
        // Its click goes on to the row's, which picks (with ⇧ for a range).
        readOnly
      />
      <FeedIcon feedId={feed.id} size={14} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1.5 text-[13px]">
          <span className="truncate">{feed.title}</span>
          {feed.error && (
            <span title={`更新できませんでした：${feed.error}`} className="shrink-0 text-danger">
              <AlertCircle size={12} />
            </span>
          )}
        </div>
        <div className="truncate text-[11px] text-dim" title={feed.url}>
          {folder && <span className="mr-1.5 rounded bg-raised px-1 py-px">{folder}</span>}
          {feed.url}
        </div>
      </div>
      <span className="flex w-14 shrink-0 justify-end" onClick={(e) => e.stopPropagation()}>
        <Switch
          checked={feed.fullText}
          onChange={(on) => void setFullTextMany([feed.id], on)}
        />
      </span>
    </div>
  );
}
