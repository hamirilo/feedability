/**
 * Operations that go through a dialog (the app's own prompt, or the OS's
 * open / save / confirm), kept out of the command table so it stays a table.
 */
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import { api, STOCKROOM_NOT_CONNECTED, type FeedInfo, type OpenRequest } from "./api";
import { useStore } from "../store";

const st = () => useStore.getState();
const fail = (e: unknown) => st().toast(String(e), true);

export function addFeedDialog() {
  st().openDialog({ kind: "addFeed" });
}

/** Subscribes, then shows the new feed. Throws so the dialog can stay open with the error. */
export async function addFeed(url: string, folderId: number | null) {
  const added = await api.addFeed(url, folderId);
  await st().reloadSidebar();
  st().setView({ kind: "feed", id: added.feedId });
  st().notify({ title: `「${added.title}」を購読しました`, detail: `${added.articles} 件の記事`, kind: "success" });
}

export function newFolderDialog() {
  st().openDialog({
    kind: "prompt",
    title: "フォルダを作成",
    label: "名前",
    value: "",
    okLabel: "作成",
    onOk: async (name) => {
      const id = await api.createFolder(name);
      await st().reloadSidebar();
      st().setView({ kind: "folder", id });
    },
  });
}

export function renameFeedDialog(id: number) {
  const feed = st().sidebar.feeds.find((f) => f.id === id);
  if (!feed) return;
  st().openDialog({
    kind: "prompt",
    title: "フィードの名前を変更",
    label: "名前（空にするとフィードの名前に戻ります）",
    value: feed.title,
    okLabel: "変更",
    onOk: async (title) => {
      await api.renameFeed(id, title);
      await st().reloadSidebar();
      void st().reload();
    },
  });
}

export function renameFolderDialog(id: number) {
  const folder = st().sidebar.folders.find((f) => f.id === id);
  if (!folder) return;
  st().openDialog({
    kind: "prompt",
    title: "フォルダの名前を変更",
    label: "名前",
    value: folder.name,
    okLabel: "変更",
    onOk: async (name) => {
      await api.renameFolder(id, name);
      await st().reloadSidebar();
    },
  });
}

/** Moves the feed to another URL (a page that names its feed works too), then shows the change. Throws so the dialog can show the reason under the field. */
export async function changeFeedUrl(id: number, url: string): Promise<FeedInfo> {
  const info = await api.changeFeedUrl(id, url);
  await Promise.all([st().reloadSidebar(), st().reload(), st().reloadIcons()]);
  st().toast("URL を変更しました");
  return info;
}

export function moveFeedDialog(feedId: number) {
  st().openDialog({ kind: "moveFeed", feedId });
}

export async function moveFeed(feedId: number, folderId: number | null) {
  try {
    await api.moveFeed(feedId, folderId);
    await st().reloadSidebar();
    void st().reload();
  } catch (e) {
    fail(e);
  }
}

/** Unsubscribing takes the feed's articles, starred ones too: asked first. */
export async function deleteFeed(id: number) {
  const feed = st().sidebar.feeds.find((f) => f.id === id);
  if (!feed) return;
  const ok = await ask(`「${feed.title}」の購読をやめますか？\n\nこのフィードの記事は、スター付きのものも含めて削除されます。`, {
    title: "購読をやめる",
    kind: "warning",
    okLabel: "購読をやめる",
    cancelLabel: "キャンセル",
  });
  if (!ok) return;
  try {
    await api.deleteFeed(id);
    await st().reloadSidebar();
    void st().reload();
    st().toast(`「${feed.title}」の購読をやめました`);
  } catch (e) {
    fail(e);
  }
}

/** The folder only; its feeds move to the top level. */
export async function deleteFolder(id: number) {
  const folder = st().sidebar.folders.find((f) => f.id === id);
  if (!folder) return;
  const ok = await ask(`フォルダ「${folder.name}」を削除しますか？\n\n中のフィードは削除されず、フォルダの外に出ます。`, {
    title: "フォルダを削除",
    kind: "warning",
    okLabel: "削除",
    cancelLabel: "キャンセル",
  });
  if (!ok) return;
  try {
    await api.deleteFolder(id);
    await st().reloadSidebar();
    void st().reload();
  } catch (e) {
    fail(e);
  }
}

export async function importOpml() {
  const path = await open({
    title: "OPML を読み込む",
    multiple: false,
    directory: false,
    filters: [{ name: "OPML", extensions: ["opml", "xml"] }],
  });
  if (typeof path !== "string") return;
  await importOpmlFrom(path);
}

async function importOpmlFrom(path: string) {
  try {
    const r = await api.importOpml(path);
    await st().reloadSidebar();
    st().notify({
      title: `${r.feedIds.length} 件のフィードを追加しました`,
      detail: r.skipped ? `${r.skipped} 件は購読済みか読めない URL のため飛ばしました` : undefined,
      kind: "success",
    });
    if (r.feedIds.length) await st().refresh(r.feedIds);
  } catch (e) {
    fail(e);
  }
}

/**
 * What the OS handed over (docs/OPEN_WITH.md). Nothing happens by itself: a
 * `feed:` link fills the add-feed dialog, an .opml file is imported once the
 * user agrees.
 */
export async function openHandedOver() {
  let requests: OpenRequest[];
  try {
    requests = await api.takeOpenRequests();
  } catch (e) {
    return fail(e);
  }
  for (const r of requests) {
    if (r.kind === "feed") {
      st().openDialog({ kind: "addFeed", url: r.url });
    } else {
      const name = r.path.split(/[\\/]/).pop();
      const ok = await ask(`「${name}」の購読を読み込みますか？\n\n購読していないフィードを追加し、更新します。`, {
        title: "OPML を読み込む",
        kind: "info",
        okLabel: "読み込む",
        cancelLabel: "キャンセル",
      });
      if (ok) await importOpmlFrom(r.path);
    }
  }
}

export async function exportOpml() {
  const path = await save({
    title: "OPML に書き出す",
    defaultPath: "feedability.opml",
    filters: [{ name: "OPML", extensions: ["opml"] }],
  });
  if (!path) return;
  try {
    const n = await api.exportOpml(path);
    st().notify({ title: `${n} 件のフィードを書き出しました`, kind: "success" });
  } catch (e) {
    fail(e);
  }
}

/**
 * The open article as Markdown on the clipboard (docs/STOCKROOM.md). The
 * Markdown is handed over as a promise inside the click or key press: WebKit
 * only lets a page write the clipboard during one, and asking Rust first
 * would end it.
 */
export async function copyMarkdown(id: number) {
  const text = api.articleMarkdown(id).then((m) => m.markdown);
  try {
    if (typeof ClipboardItem !== "undefined" && ClipboardItem.supports?.("text/plain") !== false) {
      const blob = text.then((t) => new Blob([t], { type: "text/plain" }));
      await navigator.clipboard.write([new ClipboardItem({ "text/plain": blob })]);
    } else {
      await navigator.clipboard.writeText(await text);
    }
    st().toast("Markdown をコピーしました");
  } catch (e) {
    fail(e instanceof DOMException ? "クリップボードに書き込めませんでした" : e);
  }
}

/**
 * The article into stockroom (docs/STOCKROOM.md). Not connected yet (or no
 * longer): stockroom's connection question first, then this again.
 */
export async function sendToStockroom(id: number) {
  const toast = st().notify({ title: "stockroom に送っています…", kind: "info" });
  try {
    const { duplicate } = await api.sendToStockroom(id);
    st().notify({ title: duplicate ? "stockroom にもうあります" : "stockroom に保存しました", kind: "success", replace: toast });
  } catch (e) {
    st().dismissToast(toast);
    if (e === STOCKROOM_NOT_CONNECTED) st().openDialog({ kind: "stockroomPair", thenSend: id });
    else fail(e);
  }
}

export async function openInBrowser(url: string) {
  try {
    await api.openUrl(url);
  } catch (e) {
    fail(e);
  }
}
