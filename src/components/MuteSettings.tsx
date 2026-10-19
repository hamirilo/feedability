/**
 * 設定 › ミュート (docs/MUTE.md): words that mark new articles read as they
 * come in, in every feed, one folder or one feed. A rule added applies at
 * once to the unread articles there, with 元に戻す in its toast.
 */
import { Plus, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api, type Rule } from "../lib/api";
import { composing } from "../lib/ime";
import { useStore, type MuteScope } from "../store";
import { RowButton, Section, Switch } from "./SettingsFrame";

const select =
  "h-7 max-w-48 rounded-md border border-line bg-raised px-2 text-xs outline-none focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/40";

const scopeValue = (s: MuteScope) => (s.kind === "all" ? "all" : `${s.kind}:${s.id}`);
const scopeOf = (v: string): MuteScope => {
  const [kind, id] = v.split(":");
  return kind === "feed" || kind === "folder" ? { kind, id: Number(id) } : { kind: "all" };
};

export function MuteSettings({ scope: initial }: { scope?: MuteScope }) {
  const feeds = useStore((s) => s.sidebar.feeds);
  const folders = useStore((s) => s.sidebar.folders);
  const addRule = useStore((s) => s.addRule);
  const toast = useStore((s) => s.toast);
  const [rules, setRules] = useState<Rule[] | null>(null);
  const [words, setWords] = useState("");
  const [scope, setScope] = useState<MuteScope>(initial ?? { kind: "all" });
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.listRules().then(setRules, (e) => toast(String(e), true));
  }, [toast]);

  const where = (r: Rule) => {
    if (r.feedId !== null) return feeds.find((f) => f.id === r.feedId)?.title ?? "（フィード）";
    if (r.folderId !== null) return `フォルダ「${folders.find((f) => f.id === r.folderId)?.name ?? ""}」`;
    return "すべてのフィード";
  };

  const add = async () => {
    if (!words.trim() || busy) return;
    setBusy(true);
    const rule = await addRule(words, scope);
    setBusy(false);
    if (rule) {
      setRules((rs) => [...(rs ?? []), rule]);
      setWords("");
    }
  };
  const setEnabled = (r: Rule, enabled: boolean) =>
    api.setRuleEnabled(r.id, enabled).then(
      () => setRules((rs) => rs?.map((x) => (x.id === r.id ? { ...x, enabled } : x)) ?? rs),
      (e) => toast(String(e), true),
    );
  const remove = (r: Rule) =>
    api.deleteRule(r.id).then(
      () => setRules((rs) => rs?.filter((x) => x.id !== r.id) ?? rs),
      (e) => toast(String(e), true),
    );

  return (
    <>
      <Section
        title="ミュートする語句"
        sub="タイトルか要約に語句を含む新しい記事を、届いたときに既読にします。空白で区切ると、すべてを含む記事だけ（検索と同じ）。大文字と小文字は区別しません"
      >
        <div className="flex flex-wrap items-center gap-2 px-3 py-2.5">
          <input
            type="text"
            value={words}
            onChange={(e) => setWords(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !composing(e)) void add();
            }}
            placeholder="語句（例：PR セール）"
            aria-label="ミュートする語句"
            spellCheck={false}
            autoComplete="off"
            className="h-7 min-w-0 flex-1 rounded-md border border-line bg-raised px-2 text-xs outline-none focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/40"
          />
          <select
            className={select}
            aria-label="ミュートする場所"
            value={scopeValue(scope)}
            onChange={(e) => setScope(scopeOf(e.target.value))}
          >
            <option value="all">すべてのフィード</option>
            {folders.length > 0 && (
              <optgroup label="フォルダ">
                {folders.map((f) => (
                  <option key={f.id} value={`folder:${f.id}`}>
                    {f.name}
                  </option>
                ))}
              </optgroup>
            )}
            {feeds.length > 0 && (
              <optgroup label="フィード">
                {feeds.map((f) => (
                  <option key={f.id} value={`feed:${f.id}`}>
                    {f.title}
                  </option>
                ))}
              </optgroup>
            )}
          </select>
          <RowButton primary onClick={() => void add()} disabled={!words.trim() || busy}>
            <Plus size={13} />
            追加
          </RowButton>
        </div>
        {rules !== null && rules.length === 0 && (
          <p className="px-3 py-4 text-center text-xs text-dim">まだありません。追加すると、今ある未読の記事にもすぐ効きます</p>
        )}
        {rules?.map((r) => (
          <div key={r.id} className={`flex items-center gap-2 px-3 py-1.5 ${r.enabled ? "" : "opacity-60"}`}>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13px]">{r.words}</div>
              <div className="truncate text-[11px] text-dim">{where(r)}</div>
            </div>
            <span title={r.enabled ? "オン" : "オフ（新しい記事に効きません）"}>
              <Switch checked={r.enabled} onChange={(on) => void setEnabled(r, on)} />
            </span>
            <button
              type="button"
              aria-label={`「${r.words}」を削除`}
              title="削除"
              onClick={() => void remove(r)}
              className="flex size-7 items-center justify-center rounded-md text-dim outline-none hover:bg-raised hover:text-danger focus-visible:ring-2 focus-visible:ring-accent"
            >
              <Trash2 size={13} />
            </button>
          </div>
        ))}
      </Section>
      <p className="mt-3 text-xs text-dim">
        既読にした記事は消えません。「未読のみ」を外すと見えます。オフにしたり削除したりしても、すでに既読にした記事は既読のままです
      </p>
    </>
  );
}
