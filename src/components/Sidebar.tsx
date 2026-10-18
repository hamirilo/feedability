/**
 * The views: すべて, スター付き, then the folders and their feeds. The buttons
 * at the top stay in place whatever is open (docs/UI.md).
 */
import { AlertCircle, ChevronRight, Clock, Folder, FolderPlus, Inbox, Plus, RefreshCw, Star } from "lucide-react";
import { useCallback, useState } from "react";
import { command, isEnabled, runCommand } from "../lib/commands";
import { sameView, type FeedRow, type View } from "../lib/api";
import { ago } from "../lib/format";
import { withKey } from "../lib/shortcuts";
import { dragRegion } from "../lib/titleBar";
import { useStore } from "../store";
import { commandItem, ContextMenu, type MenuState } from "./ContextMenu";
import { FeedIcon } from "./FeedIcon";
import { ResizeHandle, usePaneWidths } from "./ResizeHandle";

function Count({ n }: { n: number }) {
  return <span className="shrink-0 text-xs tabular-nums text-dim">{n > 0 ? n : ""}</span>;
}

function Entry({
  view,
  icon,
  label,
  count,
  depth = 0,
  title,
  onContextMenu,
  after,
}: {
  view: View;
  icon: React.ReactNode;
  label: string;
  count: number;
  depth?: number;
  title?: string;
  onContextMenu?: (e: React.MouseEvent) => void;
  after?: React.ReactNode;
}) {
  const active = useStore((s) => sameView(s.view, view));
  const setView = useStore((s) => s.setView);
  return (
    <button
      onClick={() => setView(view)}
      onContextMenu={onContextMenu}
      title={title}
      className={`flex w-full items-center gap-2 rounded-md py-1 pr-2 text-left ${
        active ? "bg-accent/25 text-fg" : "text-fg/90 hover:bg-hover"
      }`}
      style={{ paddingLeft: 8 + depth * 16 }}
    >
      {icon}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {after}
      <Count n={count} />
    </button>
  );
}

function IconButton({ id, icon, label }: { id: string; icon: React.ReactNode; label: string }) {
  const enabled = useStore((s) => isEnabled(command(id), s));
  return (
    <button
      onClick={() => runCommand(id)}
      disabled={!enabled}
      title={withKey(label, id)}
      aria-label={label}
      className="rounded-md p-1.5 text-dim hover:bg-hover hover:text-fg disabled:opacity-40"
    >
      {icon}
    </button>
  );
}

/** The red mark of a feed that failed: a click opens the feed's information, with the full reason. */
function ErrorMark({ feedId }: { feedId: number }) {
  const setView = useStore((s) => s.setView);
  // Opens the feed first, as the right-click menu does: the command acts on the open feed.
  const open = () => {
    setView({ kind: "feed", id: feedId });
    runCommand("feed.info");
  };
  // Not a <button>: it sits inside the row's button.
  return (
    <span
      role="button"
      tabIndex={0}
      aria-label="フィードの情報を開く"
      title="更新できませんでした。クリックで詳細"
      onClick={(e) => {
        e.stopPropagation();
        open();
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          e.stopPropagation();
          open();
        }
      }}
      className="shrink-0 rounded text-danger hover:brightness-125"
    >
      <AlertCircle size={13} />
    </span>
  );
}

function FeedEntry({ feed, depth, onMenu }: { feed: FeedRow; depth: number; onMenu: (e: React.MouseEvent, f: FeedRow) => void }) {
  const tip = feed.error
    ? `${feed.url}\n更新できませんでした：${feed.error}`
    : `${feed.url}${feed.fetchedAt ? `\n${ago(feed.fetchedAt)}に更新` : ""}`;
  return (
    <Entry
      view={{ kind: "feed", id: feed.id }}
      icon={<FeedIcon feedId={feed.id} size={14} />}
      label={feed.title}
      count={feed.unread}
      depth={depth}
      title={tip}
      onContextMenu={(e) => onMenu(e, feed)}
      after={feed.error ? <ErrorMark feedId={feed.id} /> : undefined}
    />
  );
}

export function Sidebar() {
  const sidebar = useStore((s) => s.sidebar);
  const collapsed = useStore((s) => s.collapsed);
  const toggleFolder = useStore((s) => s.toggleFolder);
  const setView = useStore((s) => s.setView);
  const [menu, setMenu] = useState<MenuState>(null);
  const closeMenu = useCallback(() => setMenu(null), []);

  // Right-click opens the feed or folder first: the menu's commands act on
  // what is open, the same as in the menu bar.
  const feedMenu = (e: React.MouseEvent, f: FeedRow) => {
    e.preventDefault();
    setView({ kind: "feed", id: f.id });
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        commandItem("feed.info"),
        "sep",
        commandItem("feed.refresh"),
        commandItem("list.markAllRead"),
        "sep",
        commandItem("feed.rename"),
        commandItem("feed.move"),
        commandItem("feed.fullText"),
        "sep",
        commandItem("feed.delete", true),
      ],
    });
  };
  const folderMenu = (e: React.MouseEvent, id: number) => {
    e.preventDefault();
    setView({ kind: "folder", id });
    setMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        commandItem("folder.refresh"),
        commandItem("list.markAllRead"),
        "sep",
        commandItem("folder.rename"),
        commandItem("folder.delete", true),
      ],
    });
  };

  const width = usePaneWidths().sidebar;
  const loose = sidebar.feeds.filter((f) => f.folderId === null);
  return (
    <aside className="relative flex h-full shrink-0 flex-col border-r border-line bg-panel" style={{ width }}>
      {/* The top: room for the window buttons on macOS, and the app's buttons. */}
      <div
        {...dragRegion(true)}
        className="flex shrink-0 items-center justify-end gap-0.5 px-2"
        style={{ height: "max(var(--titlebar), 44px)", paddingLeft: "calc(var(--traffic-lights) + 8px)" }}
      >
        <IconButton id="feed.add" icon={<Plus size={16} />} label="フィードを追加" />
        <IconButton id="folder.new" icon={<FolderPlus size={16} />} label="フォルダを作成" />
        <IconButton id="feed.refreshAll" icon={<RefreshCw size={15} />} label="すべて更新" />
      </div>
      <nav className="min-h-0 flex-1 overflow-y-auto px-2 pb-4">
        <Entry view={{ kind: "all" }} icon={<Inbox size={14} className="shrink-0 text-dim" />} label="すべての記事" count={sidebar.unread} />
        <Entry view={{ kind: "later" }} icon={<Clock size={14} className="shrink-0 text-accent" />} label="あとで読む" count={sidebar.later} />
        <Entry view={{ kind: "starred" }} icon={<Star size={14} className="shrink-0 text-star" />} label="スター付き" count={sidebar.starred} />
        <div className="mt-4 mb-1 px-2 text-[11px] font-semibold text-dim">フィード</div>
        {sidebar.feeds.length === 0 && sidebar.folders.length === 0 && (
          <p className="px-2 py-1 text-xs leading-relaxed text-dim">まだ購読していません。＋ から追加します。</p>
        )}
        {sidebar.folders.map((folder) => {
          const open = !collapsed.has(folder.id);
          return (
            <div key={folder.id}>
              <div className="flex items-center">
                <button
                  onClick={() => toggleFolder(folder.id)}
                  aria-label={open ? "閉じる" : "開く"}
                  className="rounded p-0.5 text-dim hover:text-fg"
                >
                  <ChevronRight size={13} className={`transition-transform ${open ? "rotate-90" : ""}`} />
                </button>
                <div className="min-w-0 flex-1">
                  <Entry
                    view={{ kind: "folder", id: folder.id }}
                    icon={<Folder size={14} className="shrink-0 text-dim" />}
                    label={folder.name}
                    count={folder.unread}
                    onContextMenu={(e) => folderMenu(e, folder.id)}
                  />
                </div>
              </div>
              {open &&
                sidebar.feeds
                  .filter((f) => f.folderId === folder.id)
                  .map((f) => <FeedEntry key={f.id} feed={f} depth={1} onMenu={feedMenu} />)}
            </div>
          );
        })}
        {loose.map((f) => (
          <FeedEntry key={f.id} feed={f} depth={0} onMenu={feedMenu} />
        ))}
      </nav>
      <ContextMenu menu={menu} onClose={closeMenu} />
      <ResizeHandle pane="sidebar" />
    </aside>
  );
}
