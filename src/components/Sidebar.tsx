/**
 * The views: すべて, スター付き, then the folders and their feeds. The buttons
 * at the top stay in place whatever is open (docs/UI.md).
 *
 * A feed is dragged onto a folder to move it there, or onto the feeds outside
 * the folders to take it out (docs/FOLDERS.md). Pointer events, not HTML5
 * drag and drop: Tauri's file drop turns that off on Windows (as stockroom).
 */
import { AlertCircle, ChevronRight, Clock, Folder, FolderPlus, Inbox, Plus, RefreshCw, Star } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { moveFeed } from "../lib/actions";
import { command, isEnabled, runCommand } from "../lib/commands";
import { sameView, type FeedRow, type View } from "../lib/api";
import { ago } from "../lib/format";
import { withKey } from "../lib/shortcuts";
import { dragRegion } from "../lib/titleBar";
import { useStore } from "../store";
import { commandItem, ContextMenu, type MenuState } from "./ContextMenu";
import { FeedIcon } from "./FeedIcon";
import { ResizeHandle, usePaneWidths } from "./ResizeHandle";

function Count({ n, over }: { n: number; over?: boolean }) {
  return <span className={`shrink-0 text-xs tabular-nums ${over ? "text-white" : "text-dim"}`}>{n > 0 ? n : ""}</span>;
}

function Entry({
  view,
  icon,
  label,
  count,
  depth = 0,
  title,
  onContextMenu,
  onPointerDown,
  over,
  after,
}: {
  view: View;
  icon: React.ReactNode;
  label: string;
  count: number;
  depth?: number;
  title?: string;
  onContextMenu?: (e: React.MouseEvent) => void;
  onPointerDown?: (e: React.PointerEvent) => void;
  /** A dragged feed would land here. */
  over?: boolean;
  after?: React.ReactNode;
}) {
  const active = useStore((s) => sameView(s.view, view));
  const setView = useStore((s) => s.setView);
  return (
    <button
      onClick={() => setView(view)}
      onContextMenu={onContextMenu}
      onPointerDown={onPointerDown}
      title={title}
      className={`flex w-full items-center gap-2 rounded-md py-1 pr-2 text-left transition-[background-color,transform] duration-150 ${
        over ? "scale-[1.03] bg-accent text-white shadow-lg shadow-accent/30" : active ? "bg-accent/25 text-fg" : "text-fg/90 hover:bg-hover"
      }`}
      style={{ paddingLeft: 8 + depth * 16 }}
    >
      {icon}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {after}
      <Count n={count} over={over} />
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
      onPointerDown={(e) => e.stopPropagation()}
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

/** "folder:<id>" or "root" (outside the folders): where a dragged feed would go. */
type DropTarget = string;

type FeedDrag = { feed: FeedRow; x: number; y: number };

/** The pointer moves this far before a press becomes a drag (a click otherwise). */
const DRAG_THRESHOLD = 5;

/** The drop target under the pointer, or null where the feed can't go (or already is). */
function targetAt(x: number, y: number, feed: FeedRow): DropTarget | null {
  const t = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop]")?.dataset.drop ?? null;
  const here = feed.folderId === null ? "root" : `folder:${feed.folderId}`;
  return t === here ? null : t;
}

function FeedEntry({
  feed,
  depth,
  onMenu,
  onDragStart,
}: {
  feed: FeedRow;
  depth: number;
  onMenu: (e: React.MouseEvent, f: FeedRow) => void;
  onDragStart: (d: FeedDrag) => void;
}) {
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
      onPointerDown={(e) => {
        if (e.button !== 0) return;
        const sx = e.clientX;
        const sy = e.clientY;
        const move = (ev: PointerEvent) => {
          if (Math.hypot(ev.clientX - sx, ev.clientY - sy) <= DRAG_THRESHOLD) return;
          stop();
          onDragStart({ feed, x: ev.clientX, y: ev.clientY });
        };
        const stop = () => {
          window.removeEventListener("pointermove", move);
          window.removeEventListener("pointerup", stop);
        };
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", stop);
      }}
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
        commandItem("feed.mute"),
        "sep",
        commandItem("feed.rename"),
        commandItem("feed.move"),
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
        commandItem("feed.mute"),
        "sep",
        commandItem("folder.rename"),
        commandItem("folder.delete", true),
      ],
    });
  };

  const [drag, setDrag] = useState<FeedDrag | null>(null);
  const [target, setTarget] = useState<DropTarget | null>(null);
  // The listeners read the drag from here: they are set up once per drag.
  const dragRef = useRef<FeedDrag | null>(null);
  const startDrag = useCallback((d: FeedDrag) => {
    dragRef.current = d;
    setDrag(d);
  }, []);
  const dragging = drag !== null;
  useEffect(() => {
    if (!dragging) return;
    const root = document.documentElement;
    root.dataset.dragging = "";
    const move = (e: PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      const next = { ...d, x: e.clientX, y: e.clientY };
      dragRef.current = next;
      setDrag(next);
      setTarget(targetAt(e.clientX, e.clientY, d.feed));
    };
    const finish = (e: PointerEvent | null) => {
      const d = dragRef.current;
      dragRef.current = null;
      setDrag(null);
      setTarget(null);
      const t = d && e ? targetAt(e.clientX, e.clientY, d.feed) : null;
      if (!d || !t) return;
      const folderId = t === "root" ? null : Number(t.slice(7));
      // The feed stays in sight: its folder opens.
      if (folderId !== null && useStore.getState().collapsed.has(folderId)) toggleFolder(folderId);
      void moveFeed(d.feed.id, folderId);
    };
    const up = (e: PointerEvent) => finish(e);
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      finish(null);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("keydown", key, true);
    return () => {
      delete root.dataset.dragging;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", key, true);
    };
  }, [dragging, toggleFolder]);

  // Holding a dragged feed over a closed folder opens it after a moment.
  useEffect(() => {
    if (!target?.startsWith("folder:")) return;
    const id = Number(target.slice(7));
    if (!collapsed.has(id)) return;
    const t = setTimeout(() => toggleFolder(id), 600);
    return () => clearTimeout(t);
  }, [target, collapsed, toggleFolder]);

  const width = usePaneWidths().sidebar;
  const loose = sidebar.feeds.filter((f) => f.folderId === null);
  const targetName =
    target === "root" ? "フォルダの外" : sidebar.folders.find((f) => `folder:${f.id}` === target)?.name;
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
      <nav className="flex min-h-0 flex-1 flex-col overflow-y-auto px-2 pb-4">
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
            // The whole folder, its feeds too, takes a dropped feed.
            <div key={folder.id} data-drop={`folder:${folder.id}`}>
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
                    icon={<Folder size={14} className={`shrink-0 ${target === `folder:${folder.id}` ? "text-white" : "text-dim"}`} />}
                    label={folder.name}
                    count={folder.unread}
                    onContextMenu={(e) => folderMenu(e, folder.id)}
                    over={target === `folder:${folder.id}`}
                  />
                </div>
              </div>
              {open &&
                sidebar.feeds
                  .filter((f) => f.folderId === folder.id)
                  .map((f) => <FeedEntry key={f.id} feed={f} depth={1} onMenu={feedMenu} onDragStart={startDrag} />)}
            </div>
          );
        })}
        {/* Outside the folders: the feeds there, and the rest of the pane below them. */}
        <div data-drop="root" className="min-h-8 flex-1">
          {drag && drag.feed.folderId !== null && (
            <div
              className={`my-1 rounded-md border border-dashed px-2 py-1.5 text-center text-xs transition-colors ${
                target === "root" ? "border-accent bg-accent text-white" : "border-line text-dim"
              }`}
            >
              ここにドロップでフォルダの外へ
            </div>
          )}
          {loose.map((f) => (
            <FeedEntry key={f.id} feed={f} depth={0} onMenu={feedMenu} onDragStart={startDrag} />
          ))}
        </div>
      </nav>
      {drag && (
        <div className="pointer-events-none fixed z-50" style={{ left: drag.x + 12, top: drag.y + 12 }}>
          <div className="flex max-w-60 items-center gap-2 rounded-md bg-raised px-3 py-1.5 shadow-xl ring-1 ring-accent">
            <FeedIcon feedId={drag.feed.id} size={14} />
            <span className="truncate">{drag.feed.title}</span>
          </div>
          <div
            className={`mt-2 w-max rounded-md px-2 py-0.5 text-xs shadow-lg ${
              targetName ? "bg-accent text-white" : "bg-black/70 text-white/70"
            }`}
          >
            {targetName ? `「${targetName}」へ` : "移動先のフォルダへドロップ"}
          </div>
        </div>
      )}
      <ContextMenu menu={menu} onClose={closeMenu} />
      <ResizeHandle pane="sidebar" />
    </aside>
  );
}
