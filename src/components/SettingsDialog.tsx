/**
 * ⌘, : the app's settings (docs/SETTINGS.md), saved to settings.json as soon
 * as they change.
 */
import { checkForUpdate, appVersion } from "../lib/update";
import { useStore } from "../store";
import { field, Modal } from "./Modal";

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
        hint="フィードから消えてからこの期間が過ぎた既読の記事を削除します。スター付きと未読の記事は残ります。"
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
            className="rounded-md border border-line px-3 py-1 hover:bg-white/10"
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
