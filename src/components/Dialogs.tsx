/**
 * The app's own dialogs (store `dialog`): adding a feed, a name, a folder to
 * move to. Confirmations go through the OS's dialog instead (actions.ts).
 */
import { useState } from "react";
import { addFeed, moveFeed } from "../lib/actions";
import { useStore, type Dialog } from "../store";
import { FeedInfoDialog } from "./FeedInfoDialog";
import { Buttons, ErrorLine, field, Modal, useSubmit } from "./Modal";
import { SettingsDialog } from "./SettingsDialog";
import { ShortcutHelp } from "./ShortcutHelp";
import { StockroomPairDialog } from "./StockroomDialogs";

export function Dialogs() {
  const dialog = useStore((s) => s.dialog);
  const close = useStore((s) => s.closeDialog);
  const open = useStore((s) => s.openDialog);
  if (!dialog) return null;
  switch (dialog.kind) {
    case "settings":
      return <SettingsDialog onClose={close} />;
    case "shortcuts":
      return <ShortcutHelp onClose={close} />;
    case "addFeed":
      return <AddFeedDialog initialUrl={dialog.url} onClose={close} />;
    case "prompt":
      return <PromptDialog d={dialog} onClose={close} />;
    case "moveFeed":
      return <MoveFeedDialog feedId={dialog.feedId} onClose={close} />;
    case "feedInfo":
      return <FeedInfoDialog feedId={dialog.feedId} onClose={close} />;
    case "stockroomPair":
      return (
        <StockroomPairDialog
          thenSend={dialog.thenSend}
          onClose={dialog.fromSettings ? () => open({ kind: "settings" }) : close}
        />
      );
  }
}

function FolderSelect({ value, onChange }: { value: number | null; onChange: (id: number | null) => void }) {
  const folders = useStore((s) => s.sidebar.folders);
  return (
    <select
      className={field}
      value={value ?? ""}
      onChange={(e) => onChange(e.target.value === "" ? null : Number(e.target.value))}
    >
      <option value="">（フォルダなし）</option>
      {folders.map((f) => (
        <option key={f.id} value={f.id}>
          {f.name}
        </option>
      ))}
    </select>
  );
}

function AddFeedDialog({ initialUrl, onClose }: { initialUrl?: string; onClose: () => void }) {
  const view = useStore((s) => s.view);
  const [url, setUrl] = useState(initialUrl ?? "");
  // Into the open folder, if a folder is open.
  const [folderId, setFolderId] = useState<number | null>(view.kind === "folder" ? view.id : null);
  const { busy, error, onSubmit, onKeyDown } = useSubmit(async () => {
    await addFeed(url, folderId);
    onClose();
  });
  return (
    <Modal title="フィードを追加" onClose={onClose}>
      <form onSubmit={onSubmit} onKeyDown={onKeyDown}>
        <label className="mb-1 block text-xs text-dim">フィードかサイトの URL</label>
        <input
          className={field}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          placeholder="https://example.com/feed"
          spellCheck={false}
          autoComplete="off"
        />
        <label className="mt-3 mb-1 block text-xs text-dim">フォルダ</label>
        <FolderSelect value={folderId} onChange={setFolderId} />
        <ErrorLine error={error} />
        <Buttons okLabel="追加" onCancel={onClose} busy={busy} disabled={!url.trim()} />
      </form>
    </Modal>
  );
}

function PromptDialog({ d, onClose }: { d: Extract<Dialog, { kind: "prompt" }>; onClose: () => void }) {
  const [value, setValue] = useState(d.value);
  const { busy, error, onSubmit, onKeyDown } = useSubmit(async () => {
    await d.onOk(value);
    onClose();
  });
  return (
    <Modal title={d.title} onClose={onClose}>
      <form onSubmit={onSubmit} onKeyDown={onKeyDown}>
        <label className="mb-1 block text-xs text-dim">{d.label}</label>
        <input
          className={field}
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onFocus={(e) => e.target.select()}
          autoComplete="off"
        />
        <ErrorLine error={error} />
        <Buttons okLabel={d.okLabel} onCancel={onClose} busy={busy} />
      </form>
    </Modal>
  );
}

function MoveFeedDialog({ feedId, onClose }: { feedId: number; onClose: () => void }) {
  const feed = useStore((s) => s.sidebar.feeds.find((f) => f.id === feedId));
  const [folderId, setFolderId] = useState<number | null>(feed?.folderId ?? null);
  const { busy, error, onSubmit } = useSubmit(async () => {
    await moveFeed(feedId, folderId);
    onClose();
  });
  return (
    <Modal title="フォルダへ移動" onClose={onClose}>
      <form onSubmit={onSubmit}>
        <label className="mb-1 block text-xs text-dim">「{feed?.title}」の移動先</label>
        <FolderSelect value={folderId} onChange={setFolderId} />
        <ErrorLine error={error} />
        <Buttons okLabel="移動" onCancel={onClose} busy={busy} />
      </form>
    </Modal>
  );
}
