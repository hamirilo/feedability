/**
 * ⌘, : the app's settings (docs/SETTINGS.md), saved to settings.json as soon
 * as they change.
 */
import type { MarkReadOn, ThemeSetting } from "../lib/api";
import { checkForUpdate, appVersion } from "../lib/update";
import { useStore } from "../store";
import { field, Modal } from "./Modal";

const THEMES: { value: ThemeSetting; label: string }[] = [
  { value: "system", label: "OS に合わせる" },
  { value: "light", label: "ライト" },
  { value: "dark", label: "ダーク" },
];

const MARK_READ: { value: MarkReadOn; label: string }[] = [
  { value: "open", label: "開いたとき" },
  { value: "delay", label: "開いて 2 秒後" },
  { value: "manual", label: "自分で既読にする（M）" },
];

const INTERVALS = [
  { value: 0, label: "自動では更新しない" },
  { value: 15, label: "15 分ごと" },
  { value: 30, label: "30 分ごと" },
  { value: 60, label: "1 時間ごと" },
  { value: 180, label: "3 時間ごと" },
];

const KEEP = [
  { value: 7, label: "7 日" },
  { value: 30, label: "30 日" },
  { value: 90, label: "90 日" },
  { value: 365, label: "1 年" },
  { value: 0, label: "削除しない" },
];

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[10rem_1fr] items-start gap-3 py-2">
      <div className="pt-1.5 text-dim">{label}</div>
      <div>
        {children}
        {hint && <p className="mt-1 text-xs leading-relaxed text-dim">{hint}</p>}
      </div>
    </div>
  );
}

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const settings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);
  return (
    <Modal title="設定" onClose={onClose} width={520}>
      <Row label="外観">
        <div className="flex rounded-md bg-bg p-0.5" role="radiogroup" aria-label="外観">
          {THEMES.map((t) => (
            <button
              key={t.value}
              role="radio"
              aria-checked={settings.theme === t.value}
              onClick={() => void setSettings({ ...settings, theme: t.value })}
              className={`flex-1 rounded px-2 py-1 ${
                settings.theme === t.value ? "bg-raised text-fg shadow" : "text-dim hover:text-fg"
              }`}
            >
              {t.label}
            </button>
          ))}
        </div>
      </Row>
      <Row label="フィードの更新">
        <select
          className={field}
          value={settings.refreshMinutes}
          onChange={(e) => void setSettings({ ...settings, refreshMinutes: Number(e.target.value) })}
        >
          {INTERVALS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </Row>
      <Row
        label="既読の記事を残す期間"
        hint="フィードから消えてからこの期間が過ぎた既読の記事を削除します。スター付き・あとで読む・未読の記事は残ります。"
      >
        <select
          className={field}
          value={settings.keepDays}
          onChange={(e) => void setSettings({ ...settings, keepDays: Number(e.target.value) })}
        >
          {KEEP.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </Row>
      <Row label="記事を既読にする" hint="2 秒後にすると、J / K で通り過ぎた記事は未読のまま残ります">
        <select
          className={field}
          value={settings.markReadOn}
          onChange={(e) => void setSettings({ ...settings, markReadOn: e.target.value as MarkReadOn })}
        >
          {MARK_READ.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
      </Row>
      <Row label="既読にしたあと">
        <label className="flex items-center gap-2 pt-1.5">
          <input
            type="checkbox"
            checked={settings.nextFeedAfterReadAll}
            onChange={(e) => void setSettings({ ...settings, nextFeedAfterReadAll: e.target.checked })}
          />
          すべて既読にしたら次のフィードへ移る
        </label>
      </Row>
      <Row label="アップデート">
        <label className="flex items-center gap-2 pt-1.5">
          <input
            type="checkbox"
            checked={settings.autoUpdate}
            onChange={(e) => void setSettings({ ...settings, autoUpdate: e.target.checked })}
          />
          起動時に確認する
        </label>
        <div className="mt-2 flex items-center gap-3">
          <button
            onClick={() => void checkForUpdate(true)}
            className="rounded-md border border-line px-3 py-1 hover:bg-hover"
          >
            今すぐ確認
          </button>
          <span className="text-xs text-dim">{appVersion() && `Feedability ${appVersion()}`}</span>
        </div>
      </Row>
      <div className="mt-4 flex justify-end">
        <button onClick={onClose} className="min-w-24 rounded-md bg-accent px-3 py-1.5 font-medium text-white hover:brightness-110">
          閉じる
        </button>
      </div>
    </Modal>
  );
}
