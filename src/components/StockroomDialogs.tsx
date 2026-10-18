/**
 * stockroom (docs/STOCKROOM.md): its connection question as this app shows
 * it, and the settings' stockroom section (connection, folder, tags).
 */
import { useEffect, useRef, useState } from "react";
import { sendToStockroom } from "../lib/actions";
import { api, STOCKROOM_NOT_CONNECTED, type StockroomChoices, type StockroomStatus } from "../lib/api";
import { useStore } from "../store";
import { field, Modal } from "./Modal";

/** Four digits, shown here and in stockroom's question, so the user can tell it is the same request. */
const newCode = () => String(Math.floor(Math.random() * 10_000)).padStart(4, "0");

/** Asks stockroom to connect and waits while the user answers there. */
export function StockroomPairDialog({ thenSend, onClose }: { thenSend?: number; onClose: () => void }) {
  const [code, setCode] = useState(newCode);
  const [error, setError] = useState<string | null>(null);
  const [waiting, setWaiting] = useState(true);
  // Read when the answer comes, not a reason to ask again.
  const after = useRef({ thenSend, onClose });
  after.current = { thenSend, onClose };
  useEffect(() => {
    if (!waiting) return;
    let stale = false;
    const ask = () =>
      api.stockroomPair(code).then(
        () => {
          if (stale) return;
          const { thenSend, onClose } = after.current;
          onClose();
          useStore.getState().toast("stockroom と接続しました");
          if (thenSend !== undefined) void sendToStockroom(thenSend);
        },
        (e) => {
          if (stale) return;
          setError(String(e));
          setWaiting(false);
        },
      );
    // Asked a moment later: a mount undone at once (React's dev double mount)
    // asks nothing, where stockroom would answer the second with "already asking".
    const timer = setTimeout(() => void ask(), 0);
    // Closed meanwhile: the answer is dropped (stockroom's question times out by itself).
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [code, waiting]);

  const again = () => {
    setError(null);
    setCode(newCode());
    setWaiting(true);
  };
  return (
    <Modal title="stockroom と接続" onClose={onClose} width={420}>
      <p className="text-dim">
        {thenSend !== undefined ? "記事を送る前に、stockroom との接続を許可してもらいます。" : ""}
        stockroom に確認が出ています。番号が同じなら、stockroom で「許可する」を押してください。
      </p>
      <div className="my-5 text-center font-mono text-4xl font-semibold tracking-[0.3em] tabular-nums">{code}</div>
      {/* The line keeps its height whether it says anything: nothing below moves. */}
      <p className={`min-h-10 text-xs ${error ? "text-danger" : "text-dim"}`}>
        {error ?? "stockroom で答えるのを待っています…（2 分まで）"}
      </p>
      <div className="mt-3 flex justify-end gap-2">
        <button type="button" onClick={onClose} className="min-w-24 rounded-md border border-line px-3 py-1.5 hover:bg-hover">
          {waiting ? "やめる" : "閉じる"}
        </button>
        <button
          type="button"
          onClick={again}
          disabled={waiting}
          className="min-w-24 rounded-md bg-accent px-3 py-1.5 font-medium text-white hover:brightness-110 disabled:opacity-50"
        >
          もう一度
        </button>
      </div>
    </Modal>
  );
}

/** The settings' stockroom section: asks stockroom for its folders and tags when it shows. */
export function StockroomSettings() {
  const dialog = useStore((s) => s.dialog);
  const openDialog = useStore((s) => s.openDialog);
  const toast = useStore((s) => s.toast);
  const [status, setStatus] = useState<StockroomStatus | null>(null);
  const [choices, setChoices] = useState<StockroomChoices | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [tags, setTags] = useState("");

  // Again whenever the settings are back on top (after the connection question).
  const onTop = dialog?.kind === "settings";
  useEffect(() => {
    if (!onTop) return;
    let stale = false;
    void (async () => {
      try {
        const s = await api.stockroomStatus();
        if (stale) return;
        setStatus(s);
        setTags(s.tags.join(", "));
        if (!s.connected) return;
        const c = await api.stockroomChoices();
        if (!stale) {
          setChoices(c);
          setProblem(null);
        }
      } catch (e) {
        if (stale) return;
        if (e === STOCKROOM_NOT_CONNECTED) setStatus(await api.stockroomStatus());
        else setProblem(String(e));
      }
    })();
    return () => {
      stale = true;
    };
  }, [onTop]);

  if (!status) return null;
  const save = async (folder: string | null, folderName: string | null, tagText: string) => {
    try {
      setStatus(await api.setStockroomTarget(folder, folderName, tagText.split(/[,、]/)));
    } catch (e) {
      toast(String(e), true);
    }
  };
  const disconnect = async () => {
    try {
      setStatus(await api.stockroomDisconnect());
      setChoices(null);
    } catch (e) {
      toast(String(e), true);
    }
  };
  const connect = () => openDialog({ kind: "stockroomPair", fromSettings: true });

  return (
    <div>
      <div className="flex items-center gap-3 pt-1.5">
        <span>{status.connected ? "接続済み" : "未接続"}</span>
        {status.connected ? (
          <>
            <button onClick={connect} className="rounded-md border border-line px-3 py-1 hover:bg-hover">
              接続し直す
            </button>
            <button onClick={() => void disconnect()} className="text-xs text-dim hover:text-fg">
              接続を解除
            </button>
          </>
        ) : (
          <button onClick={connect} className="rounded-md border border-line px-3 py-1 hover:bg-hover">
            接続する
          </button>
        )}
      </div>
      {status.connected && (
        <div className="mt-3 space-y-2">
          <label className="block">
            <span className="mb-1 block text-xs text-dim">送り先のフォルダ</span>
            <select
              className={field}
              value={status.folder ?? ""}
              disabled={!choices}
              onChange={(e) => {
                const f = choices?.folders.find((x) => x.id === e.target.value);
                void save(f?.id ?? null, f?.name ?? null, tags);
              }}
            >
              <option value="">未分類</option>
              {/* The saved one, while stockroom's list isn't in (or stockroom is closed). */}
              {!choices && status.folder && <option value={status.folder}>{status.folderName ?? status.folder}</option>}
              {choices?.folders.map((f) => (
                <option key={f.id} value={f.id}>
                  {f.name}
                </option>
              ))}
            </select>
          </label>
          <label className="block">
            <span className="mb-1 block text-xs text-dim">付けるタグ（「,」で区切る）</span>
            <input
              className={field}
              value={tags}
              list="stockroom-tags"
              placeholder="なし"
              onChange={(e) => setTags(e.target.value)}
              onBlur={() => void save(status.folder, status.folderName, tags)}
              spellCheck={false}
              autoComplete="off"
            />
            <datalist id="stockroom-tags">
              {choices?.tags.map((t) => <option key={t} value={t} />)}
            </datalist>
          </label>
          {problem && <p className="text-xs text-danger">{problem}</p>}
        </div>
      )}
    </div>
  );
}
