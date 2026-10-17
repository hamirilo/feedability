/**
 * The keyboard shortcuts for the shortcut list (?), and how a key reads on
 * this platform. Rows of operations come from the command table
 * (commands.ts), which the menus and the key handler use too.
 */
import { command } from "./commands";

export type ShortcutRow = { label: string; keys: string[] };
export type ShortcutSection = { title: string; rows: ShortcutRow[] };

const cmd = (id: string, label?: string): ShortcutRow => {
  const c = command(id);
  return { label: label ?? c.title, keys: c.keys ?? [] };
};

let sections: ShortcutSection[] | null = null;

/** Built on first use: the command table must be loaded first. */
export function shortcutSections(): ShortcutSection[] {
  sections ??= [
    {
      title: "全体",
      rows: [
        cmd("feed.add"),
        cmd("folder.new"),
        cmd("feed.refreshAll"),
        cmd("feed.refresh"),
        cmd("feed.info"),
        cmd("view.all"),
        cmd("view.later"),
        cmd("view.starred"),
        cmd("edit.search"),
        cmd("app.settings", "設定"),
        cmd("help.shortcuts", "ショートカット一覧"),
      ],
    },
    {
      title: "一覧",
      rows: [
        cmd("go.next"),
        cmd("go.prev"),
        cmd("list.unreadOnly", "未読のみ表示の切り替え"),
        cmd("list.markAllRead"),
      ],
    },
    {
      title: "記事",
      rows: [
        cmd("article.page"),
        cmd("article.read", "既読 / 未読の切り替え"),
        cmd("article.star", "スターを付ける / 外す"),
        cmd("article.later", "あとで読むに入れる / 外す"),
        cmd("article.open"),
      ],
    },
    {
      title: "ダイアログ・入力",
      rows: [
        { label: "閉じる・入力をやめる", keys: ["Esc"] },
        { label: "決定", keys: ["Enter"] },
      ],
    },
  ];
  return sections;
}

export const isMac = navigator.userAgent.includes("Mac");

const MAC: Record<string, string> = {
  Mod: "⌘",
  Alt: "⌥",
  Shift: "⇧",
  // "↩" is drawn as an emoji.
  Enter: "Return",
};
const OTHER: Record<string, string> = { Mod: "Ctrl", Alt: "Alt", Shift: "Shift" };
const BOTH: Record<string, string> = { ArrowDown: "↓", ArrowUp: "↑", Space: "Space", Esc: "Esc" };

/** The parts of a combo as shown, each drawn as a key. */
export function keyParts(combo: string): string[] {
  return combo.split("+").map((p) => BOTH[p] ?? (isMac ? MAC : OTHER)[p] ?? p);
}

/** "⌘⇧R" / "Ctrl+Shift+R": how a combo reads (menus, tooltips). */
export const comboText = (combo: string) => keyParts(combo).join(isMac ? "" : "+");

/** "スター（S）": a button's tooltip with its first key. */
export function withKey(label: string, id: string): string {
  const key = command(id).keys?.[0];
  return key ? `${label}（${comboText(key)}）` : label;
}
