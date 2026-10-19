/**
 * ⌘I : a feed's information, its URL and its settings (docs/FEED_INFO.md).
 * The URL can be changed here: the new address is checked first (a page that
 * names its feed works too), and the reason it was refused shows under the
 * field. The settings (全文取得) are only here, apart from the information:
 * the menus hold actions, not switches.
 */
import { useEffect, useState } from "react";
import { changeFeedUrl, openInBrowser } from "../lib/actions";
import { api, type FeedInfo } from "../lib/api";
import { dayOnly, fullDate } from "../lib/format";
import { useStore } from "../store";
import { ErrorLine, field, Modal, useSubmit } from "./Modal";

/** The labels are small and grey, the values the size of body text, so the two columns don't read alike. */
const LABEL = "text-xs text-dim";

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[7.5rem_1fr] items-baseline gap-3 py-2">
      <div className={LABEL}>{label}</div>
      <div className="min-w-0 break-words text-fg select-text">{children}</div>
    </div>
  );
}

/** What isn't there: lighter than a value, and unlike a label. */
const None = ({ children }: { children: React.ReactNode }) => <span className="text-fg/45">{children}</span>;

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

  const toggleFullText = useStore((s) => s.toggleFullText);
  const fullText = row?.fullText ?? info?.fullText ?? false;

  const folder = info?.folderId != null ? folders.find((f) => f.id === info.folderId)?.name : null;
  return (
    <Modal title="フィードの情報と設定" onClose={onClose} width={520}>
      <form onSubmit={onSubmit} onKeyDown={onKeyDown} className="pb-1">
        <label className={`mb-1.5 block ${LABEL}`} htmlFor="feed-url">
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
          {/* Below the reserved error line of the field: no more space above. */}
          <div className="divide-y divide-line/60 rounded-lg border border-line bg-bg/60 px-3">
            <Row label="名前">
              <span className="font-semibold">{info.title}</span>
              {info.originalTitle !== info.title && <span className="ml-2 text-xs text-dim">元の名前：{info.originalTitle}</span>}
            </Row>
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
                <None>なし</None>
              )}
            </Row>
            <Row label="フォルダ">{folder ?? <None>なし</None>}</Row>
            <Row label="追加した日">{dayOnly(info.addedAt)}</Row>
            <Row label="最後に更新した日時">{info.fetchedAt ? fullDate(info.fetchedAt) : <None>まだ更新していません</None>}</Row>
            <Row label="記事">
              {info.articles} 件（未読 {info.unread} 件）
            </Row>
          </div>
          {info.error && (
            <div className="mt-3 rounded-md border border-danger/40 bg-danger/10 p-3 text-xs">
              <div className="font-semibold text-danger">更新できませんでした</div>
              <p className="mt-1 break-words whitespace-pre-wrap text-fg select-text">{info.error}</p>
            </div>
          )}
          <h3 className="mt-4 mb-1.5 text-xs font-semibold text-dim">設定</h3>
          <div className="rounded-lg border border-line bg-bg/60 px-3">
            <Row label="全文取得">
              <label className="flex items-start gap-2">
                <input type="checkbox" className="mt-1" checked={fullText} onChange={() => void toggleFullText(feedId)} />
                <span>
                  記事のページから本文を取得する
                  <span className="mt-0.5 block text-xs text-dim">要約だけのフィード向け。新しい記事は更新のときに、それまでの記事は開いたときに取得します</span>
                </span>
              </label>
            </Row>
          </div>
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
