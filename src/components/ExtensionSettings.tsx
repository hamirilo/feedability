/**
 * The browser extension (docs/EXTENSION.md): the settings' section that
 * turns the receiver on (127.0.0.1:41630, off by default) and writes the
 * extension out, and the question an extension's request to connect brings.
 */
import { FolderOpen } from "lucide-react";
import { useEffect, useState } from "react";
import { api, type ExtensionPairRequest, type ExtensionStatus } from "../lib/api";
import { useStore } from "../store";
import { Row, RowButton, Section, SwitchRow } from "./SettingsFrame";

export function ExtensionSettings() {
  const toast = useStore((s) => s.toast);
  const [status, setStatus] = useState<ExtensionStatus | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    api.extensionStatus().then(setStatus, (e) => toast(String(e), true));
  }, [toast]);

  const act = async (f: () => Promise<ExtensionStatus | string>) => {
    setBusy(true);
    try {
      const r = await f();
      setStatus(typeof r === "string" ? await api.extensionStatus() : r);
    } catch (e) {
      toast(String(e), true);
    } finally {
      setBusy(false);
    }
  };
  const reset = () =>
    act(async () => {
      const s = await api.resetExtensionToken();
      toast("接続を解除しました。拡張機能はもう一度「アプリと接続」が要ります");
      return s;
    });

  return (
    <>
      {/* The frame's header names the page. */}
      <p className="mb-4 text-xs text-dim">
        Chrome（Microsoft Edge でも可）の拡張機能から、見ているページのフィードを購読したり、ページをあとで読むに入れたりできます。
      </p>
      <Section>
        <SwitchRow
          label={<span className="font-medium">ブラウザ拡張からの購読・あとで読むを受け付ける</span>}
          checked={status?.enabled ?? false}
          disabled={!status || busy}
          onChange={(v) => void act(() => api.setExtensionReceiver(v))}
          hint={
            !status ? (
              "確認中…"
            ) : status.error ? (
              <span className="text-danger">{status.error}</span>
            ) : status.running ? (
              `受付中（127.0.0.1:${status.port}）。このパソコンの中の、接続を許可した拡張機能からだけ受け付けます`
            ) : (
              "オフ。アプリの起動中だけ受け付けます"
            )
          }
        />
      </Section>
      <Section title="拡張機能">
        <Row
          label="拡張機能のフォルダ"
          hint={
            status?.folder ? (
              <>
                Chrome の <code>chrome://extensions</code> で「デベロッパー モード」をオンにし、「パッケージ化されていない拡張機能を読み込む」でこのフォルダを選びます
                <span className="mt-1 block break-all text-fg/80 select-text">{status.folder}</span>
              </>
            ) : (
              "接続の鍵を入れた拡張機能を書き出し、フォルダを表示します"
            )
          }
        >
          <RowButton onClick={() => void act(() => api.installExtension())} disabled={busy}>
            <FolderOpen size={12} />
            {status?.folder ? "もう一度用意する" : "用意して表示"}
          </RowButton>
        </Row>
        <Row label="すべての拡張機能との接続を解除" hint="接続の鍵を作り直します。拡張機能は、もう一度「アプリと接続」が要ります">
          <RowButton onClick={() => void reset()} disabled={busy}>
            解除
          </RowButton>
        </Row>
      </Section>
    </>
  );
}

function answer(req: ExtensionPairRequest, allow: boolean) {
  const { setExtensionPair, toast } = useStore.getState();
  setExtensionPair(null);
  api.answerExtensionPair(req.id, allow).catch((e) => toast(String(e), true));
  if (allow) toast("拡張機能と接続しました");
}

/** A browser extension asks to connect (POST /pair): allow or refuse. Over every dialog. */
export function ExtensionPairDialog() {
  const req = useStore((s) => s.extensionPair);
  useEffect(() => {
    if (!req) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") answer(req, false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [req]);
  if (!req) return null;

  return (
    <div data-modal className="fixed inset-0 z-[60] flex animate-fade-in items-center justify-center bg-black/50">
      <div role="dialog" aria-label="拡張機能の接続" className="w-[400px] max-w-[92vw] rounded-xl border border-line bg-raised p-5 text-center shadow-2xl shadow-black/60">
        <div className="text-base font-semibold">ブラウザの拡張機能が接続を求めています</div>
        <p className="mt-1 text-xs text-dim">
          拡張機能の画面に出ている番号と同じなら「許可する」を押してください。許可すると、その拡張機能からフィードの購読と、あとで読むへの追加ができるようになります。
        </p>
        <div className="my-4 font-mono text-4xl font-semibold tracking-[0.3em] tabular-nums">{req.code}</div>
        <p className="-mt-2 mb-4 text-[11px] text-dim">
          拡張機能の ID：<span className="font-mono select-all">{req.extension}</span>
          <br />
          心当たりがなければ「許可しない」を押してください（ID は chrome://extensions で確かめられます）
        </p>
        <div className="flex justify-center gap-2">
          <button onClick={() => answer(req, false)} className="min-w-24 rounded-md border border-line px-3 py-1.5 hover:bg-hover">
            許可しない
          </button>
          <button autoFocus onClick={() => answer(req, true)} className="min-w-24 rounded-md bg-accent px-3 py-1.5 font-medium text-white hover:brightness-110">
            許可する
          </button>
        </div>
      </div>
    </div>
  );
}
