/**
 * ⌘I : a feed's information and its URL (docs/FEED_INFO.md). The URL can be
 * changed here: the new address is checked first (a page that names its feed
 * works too), and the reason it was refused shows under the field.
 */
import { useEffect, useState } from "react";
import { changeFeedUrl, openInBrowser } from "../lib/actions";
import { api, type FeedInfo } from "../lib/api";
import { dayOnly, fullDate } from "../lib/format";
import { useStore } from "../store";
import { ErrorLine, field, Modal, useSubmit } from "./Modal";

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[8.5rem_1fr] items-baseline gap-3 py-1.5">
      <div className="text-dim">{label}</div>
      <div className="min-w-0 break-words select-text">{children}</div>
    </div>
  );
}

export function FeedInfoDialog({ feedId, onClose }: { feedId: number; onClose: () => void }) {
  // The sidebar's row is there at once: the field is on screen before the rest arrives.
  const row = useStore((s) => s.sidebar.feeds.find((f) => f.id === feedId));
  const folders = useStore((s) => s.sidebar.folders);
  const [info, setInfo] = useState<FeedInfo | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [url, setUrl] = useState(row?.url ?? "");

  useEffect(() => {
    let stale = false;
    api.getFeedInfo(feedId).then(
      (i) => !stale && setInfo(i),
      (e) => !stale && setLoadError(String(e)),
    );
    return () => {
      stale = true;
    };
  }, [feedId]);

  const current = info?.url ?? row?.url ?? "";
  const changed = url.trim() !== "" && url.trim() !== current;
  const { busy, error, onSubmit, onKeyDown } = useSubmit(async () => {
    const next = await changeFeedUrl(feedId, url);
    setInfo(next);
    setUrl(next.url);
  });

  const folder = info?.folderId != null ? folders.find((f) => f.id === info.folderId)?.name : null;
  return (
    <Modal title="フィードの情報" onClose={onClose} width={520}>
      {info && (
        <Row label="名前">
          {info.title}
          {info.originalTitle !== info.title && <span className="ml-2 text-xs text-dim">元の名前：{info.originalTitle}</span>}
        </Row>
      )}
      <form onSubmit={onSubmit} onKeyDown={onKeyDown} className="py-1.5">
        <label className="mb-1 block text-dim" htmlFor="feed-url">
          URL
        </label>
        <div className="flex gap-2">
          <input
            id="feed-url"
            className={field}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            spellCheck={false}
            autoComplete="off"
          />
          <button
            type="submit"
            disabled={busy || !changed}
            // Wide enough for "確認しています…": the field doesn't change size while it works.
            className="min-w-28 shrink-0 rounded-md bg-accent px-3 py-1.5 font-medium text-white hover:brightness-110 disabled:opacity-50"
          >
            {busy ? "確認しています…" : "変更"}
          </button>
        </div>
        <ErrorLine error={error} />
      </form>
      {loadError && <p className="text-xs text-danger">{loadError}</p>}
      {info && (
        <>
          <Row label="サイト">
            {info.siteUrl ? (
              <a
                href={info.siteUrl}
                onClick={(e) => {
                  e.preventDefault();
                  void openInBrowser(info.siteUrl!);
                }}
                className="text-accent hover:underline"
              >
                {info.siteUrl}
              </a>
            ) : (
              <span className="text-dim">なし</span>
            )}
          </Row>
          <Row label="フォルダ">{folder ?? <span className="text-dim">（フォルダなし）</span>}</Row>
          <Row label="追加した日">{dayOnly(info.addedAt)}</Row>
          <Row label="最後に更新した日時">{info.fetchedAt ? fullDate(info.fetchedAt) : <span className="text-dim">まだ更新していません</span>}</Row>
          <Row label="記事">
            {info.articles} 件（未読 {info.unread} 件）
          </Row>
          {info.error && (
            <div className="mt-3 rounded-md border border-danger/40 bg-danger/10 p-3 text-xs">
              <div className="font-semibold text-danger">更新できませんでした</div>
              <p className="mt-1 break-words whitespace-pre-wrap text-fg select-text">{info.error}</p>
            </div>
          )}
        </>
      )}
      <div className="mt-4 flex justify-end">
        <button type="button" onClick={onClose} className="min-w-24 rounded-md bg-accent px-3 py-1.5 font-medium text-white hover:brightness-110">
          閉じる
        </button>
      </div>
    </Modal>
  );
}
