/**
 * ⌘, : the app's settings (docs/SETTINGS.md), in the frame every tool shares
 * (SettingsFrame.tsx, desktop-platform UI §9): pages on the left, sections
 * of rows on the right, saved as they change. ⌘, opens the page seen last.
 */
import { Archive, BookOpen, Puzzle, Settings } from "lucide-react";
import type { MarkReadOn, ThemeSetting } from "../lib/api";
import { useStore, type SettingsPageId } from "../store";
import { ExtensionSettings } from "./ExtensionSettings";
import { RadioRows, Row, Section, SettingsFrame, SwitchRow, type SettingsGroup } from "./SettingsFrame";
import { StockroomSettings } from "./StockroomDialogs";
import { UpdateSection } from "./UpdateSection";

const GROUPS: SettingsGroup<SettingsPageId>[] = [
  {
    pages: [
      { id: "general", label: "一般", icon: <Settings size={15} /> },
      { id: "reading", label: "記事", icon: <BookOpen size={15} /> },
      { id: "extension", label: "ブラウザ拡張", icon: <Puzzle size={15} /> },
      { id: "stockroom", label: "stockroom", icon: <Archive size={15} /> },
    ],
  },
];

const THEMES: { value: ThemeSetting; label: string; hint?: string }[] = [
  { value: "system", label: "OS に合わせる", hint: "OS のライト / ダークの切り替えに付いていきます" },
  { value: "light", label: "ライト" },
  { value: "dark", label: "ダーク" },
];

/** 表示の大きさ (docs/UI_SCALE.md); within commands.rs UI_SCALE_MIN..MAX. */
const UI_SCALES = [80, 90, 100, 110, 125, 150];

const MARK_READ: { value: MarkReadOn; label: string; hint?: string }[] = [
  { value: "open", label: "開いたとき" },
  { value: "delay", label: "開いて 2 秒後", hint: "J / K で通り過ぎた記事は未読のまま残ります" },
  { value: "manual", label: "自分で既読にする", hint: "M で既読にします" },
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

/** A choice from a list, on the right of a row. */
const select =
  "h-7 rounded-md border border-line bg-raised px-2 text-xs outline-none focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/40";

export function SettingsDialog({ page, onClose }: { page: SettingsPageId; onClose: () => void }) {
  const openSettings = useStore((s) => s.openSettings);
  return (
    <SettingsFrame groups={GROUPS} page={page} onPage={openSettings} onClose={onClose}>
      {page === "general" && <GeneralPage />}
      {page === "reading" && <ReadingPage />}
      {page === "extension" && <ExtensionSettings />}
      {page === "stockroom" && <StockroomSettings />}
    </SettingsFrame>
  );
}

function GeneralPage() {
  const settings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);
  return (
    <>
      <Section title="外観">
        <RadioRows
          name="theme"
          value={settings.theme}
          options={THEMES}
          onChange={(theme) => void setSettings({ ...settings, theme })}
        />
        <Row label="表示の大きさ" hint="サイドバー・一覧・本文など、画面全体の文字と部品の大きさ。本文の文字だけを変えるときは本文の上の「Aa」">
          <select
            className={select}
            aria-label="表示の大きさ"
            value={settings.uiScale}
            onChange={(e) => void setSettings({ ...settings, uiScale: Number(e.target.value) })}
          >
            {/* A hand-edited value between the steps still shows. */}
            {(UI_SCALES.includes(settings.uiScale) ? UI_SCALES : [...UI_SCALES, settings.uiScale].sort((a, b) => a - b)).map((v) => (
              <option key={v} value={v}>
                {v} %{v === 100 ? "（標準）" : ""}
              </option>
            ))}
          </select>
        </Row>
      </Section>
      <Section title="フィード">
        <Row label="フィードの更新" hint="アプリを開いている間、この間隔ですべてのフィードを更新します">
          <select
            className={select}
            aria-label="フィードの更新"
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
          hint="フィードから消えてからこの期間が過ぎた既読の記事を削除します。スター付き・あとで読む・未読の記事は残ります"
        >
          <select
            className={select}
            aria-label="既読の記事を残す期間"
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
      </Section>
      {/* Last on 一般, in every tool (UI §9.1). */}
      <UpdateSection autoUpdate={settings.autoUpdate} onAutoUpdate={(autoUpdate) => void setSettings({ ...settings, autoUpdate })} />
    </>
  );
}

function ReadingPage() {
  const settings = useStore((s) => s.settings);
  const setSettings = useStore((s) => s.setSettings);
  return (
    <>
      <Section title="記事を既読にする">
        <RadioRows
          name="markReadOn"
          value={settings.markReadOn}
          options={MARK_READ}
          onChange={(markReadOn) => void setSettings({ ...settings, markReadOn })}
        />
      </Section>
      <Section title="一覧">
        <SwitchRow
          label="すべて既読にしたら次のフィードへ移る"
          hint="一覧の「すべて既読」のあと、未読のある次のフィードを開きます"
          checked={settings.nextFeedAfterReadAll}
          onChange={(nextFeedAfterReadAll) => void setSettings({ ...settings, nextFeedAfterReadAll })}
        />
      </Section>
    </>
  );
}
