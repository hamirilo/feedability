/**
 * The menu bar, made from the command table: the app's menus at the top of
 * the screen on macOS, the window's menu on Windows.
 *
 * Keys: ⌘ / Ctrl combos are the items' accelerators. Single keys (J, K, M,
 * S, V, Space) are not, since they must still type into text fields; they
 * are only shown (Windows: right-aligned after a tab, macOS: after the name).
 *
 * The menu is built once; afterwards only the names, greyed-out and check
 * states are updated, at most once a frame.
 */
import { isTauri } from "@tauri-apps/api/core";
import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu } from "@tauri-apps/api/menu";
import { command, isEnabled, labelOf, runCommand } from "./commands";
import { warn } from "./log";
import { comboText, isMac } from "./shortcuts";
import { useStore, type State } from "../store";

type Item = MenuItem | CheckMenuItem | Submenu | PredefinedMenuItem;

/** A live item: how to read its state, and what it showed last. */
interface Entry {
  item: MenuItem | CheckMenuItem;
  text: (s: State) => string;
  enabled: (s: State) => boolean;
  checked?: (s: State) => boolean;
  shown: { text: string; enabled: boolean; checked?: boolean };
}

const st = () => useStore.getState();

/** "Mod+Shift+N" → "CmdOrCtrl+Shift+N". */
function accelerator(key: string): string {
  const names: Record<string, string> = { Mod: "CmdOrCtrl", ",": "Comma" };
  return key
    .split("+")
    .map((p) => names[p] ?? p)
    .join("+");
}

/** The label with a key that is shown but not an accelerator. */
function withKey(text: string, key: string | undefined): string {
  if (!key) return text;
  return isMac ? `${text}（${comboText(key)}）` : `${text}\t${comboText(key)}`;
}

let entries: Entry[] = [];

/** A menu item for a command. */
async function cmd(id: string): Promise<MenuItem | CheckMenuItem> {
  const c = command(id);
  const s = st();
  const combo = c.keys?.find((k) => k.includes("Mod+"));
  const accel = combo ? accelerator(combo) : undefined;
  const text = (x: State) => (accel ? labelOf(c, x) : withKey(labelOf(c, x), c.keys?.[0]));
  const enabled = (x: State) => isEnabled(c, x);
  const base = { text: text(s), enabled: enabled(s), accelerator: accel, action: () => runCommand(id) };
  const item = c.checked ? await CheckMenuItem.new({ ...base, checked: c.checked(s) }) : await MenuItem.new(base);
  entries.push({
    item,
    text,
    enabled,
    checked: c.checked,
    shown: { text: base.text, enabled: base.enabled, checked: c.checked?.(s) },
  });
  return item;
}

const sep = () => PredefinedMenuItem.new({ item: "Separator" });

async function submenu(text: string, items: Promise<Item>[]): Promise<Submenu> {
  return Submenu.new({ text, items: await Promise.all(items) });
}

async function build(): Promise<Menu> {
  entries = [];
  const appMenu = isMac
    ? [
        submenu("Feedability", [
          PredefinedMenuItem.new({ item: { About: null }, text: "Feedability について" }),
          cmd("app.checkUpdate"),
          sep(),
          cmd("app.settings"),
          sep(),
          PredefinedMenuItem.new({ item: "Services", text: "サービス" }),
          sep(),
          PredefinedMenuItem.new({ item: "Hide", text: "Feedability を隠す" }),
          PredefinedMenuItem.new({ item: "HideOthers", text: "ほかを隠す" }),
          PredefinedMenuItem.new({ item: "ShowAll", text: "すべてを表示" }),
          sep(),
          PredefinedMenuItem.new({ item: "Quit", text: "Feedability を終了" }),
        ]),
      ]
    : [];

  const file = submenu("ファイル", [
    cmd("feed.add"),
    cmd("folder.new"),
    sep(),
    cmd("opml.import"),
    cmd("opml.export"),
    ...(isMac ? [] : [sep(), cmd("app.settings"), sep(), PredefinedMenuItem.new({ item: "Quit", text: "終了" })]),
  ]);

  const edit = submenu("編集", [
    ...(isMac
      ? [
          PredefinedMenuItem.new({ item: "Undo", text: "取り消す" }),
          PredefinedMenuItem.new({ item: "Redo", text: "やり直す" }),
          sep(),
          PredefinedMenuItem.new({ item: "Cut", text: "カット" }),
          PredefinedMenuItem.new({ item: "Copy", text: "コピー" }),
          PredefinedMenuItem.new({ item: "Paste", text: "ペースト" }),
          PredefinedMenuItem.new({ item: "SelectAll", text: "すべてを選択" }),
          sep(),
        ]
      : []),
    cmd("edit.search"),
  ]);

  const feed = submenu("フィード", [
    cmd("feed.refreshAll"),
    cmd("feed.refresh"),
    sep(),
    cmd("feed.info"),
    cmd("feed.rename"),
    cmd("feed.move"),
    cmd("feed.delete"),
    sep(),
    cmd("folder.refresh"),
    cmd("folder.rename"),
    cmd("folder.delete"),
  ]);

  const article = submenu("記事", [
    cmd("go.next"),
    cmd("go.prev"),
    cmd("go.nextUnread"),
    sep(),
    cmd("go.nextFeed"),
    cmd("go.prevFeed"),
    sep(),
    cmd("article.read"),
    cmd("article.star"),
    cmd("article.later"),
    cmd("article.open"),
    sep(),
    cmd("list.markAllRead"),
    cmd("list.markAboveRead"),
    cmd("list.markBelowRead"),
  ]);

  const view = submenu("表示", [cmd("view.all"), cmd("view.later"), cmd("view.starred"), sep(), cmd("list.unreadOnly"), cmd("list.oldestFirst")]);

  const windowMenu = isMac
    ? [
        submenu("ウインドウ", [
          PredefinedMenuItem.new({ item: "Minimize", text: "しまう" }),
          PredefinedMenuItem.new({ item: "Maximize", text: "拡大／縮小" }),
          PredefinedMenuItem.new({ item: "Fullscreen", text: "フルスクリーンにする" }),
          sep(),
          PredefinedMenuItem.new({ item: "BringAllToFront", text: "すべてを手前に移動" }),
        ]),
      ]
    : [];

  const help = submenu("ヘルプ", [
    cmd("help.shortcuts"),
    sep(),
    cmd("help.log"),
    ...(isMac ? [] : [sep(), cmd("app.checkUpdate"), PredefinedMenuItem.new({ item: { About: null }, text: "バージョン情報" })]),
  ]);

  return Menu.new({ items: await Promise.all([...appMenu, file, edit, feed, article, view, ...windowMenu, help]) });
}

let frame = 0;

/** Brings names, greyed-out and check states up to date (once a frame). */
function sync() {
  if (frame) return;
  frame = requestAnimationFrame(() => {
    frame = 0;
    const s = st();
    for (const e of entries) {
      const text = e.text(s);
      const enabled = e.enabled(s);
      if (text !== e.shown.text) void e.item.setText(text);
      if (enabled !== e.shown.enabled) void e.item.setEnabled(enabled);
      if (e.checked && e.item instanceof CheckMenuItem) {
        const checked = e.checked(s);
        if (checked !== e.shown.checked) void e.item.setChecked(checked);
        e.shown.checked = checked;
      }
      e.shown.text = text;
      e.shown.enabled = enabled;
    }
  });
}

/** Sets up the menu bar (in the app only; the browser preview has none). */
export function installMenuBar(): () => void {
  if (!isTauri()) return () => {};
  build()
    .then((menu) => menu.setAsAppMenu())
    .then(sync)
    .catch((e) => warn("menu bar", e));
  return useStore.subscribe(sync);
}
