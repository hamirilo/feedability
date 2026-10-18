import { useEffect } from "react";
import { ArticleList } from "./components/ArticleList";
import { Dialogs } from "./components/Dialogs";
import { ExtensionPairDialog } from "./components/ExtensionSettings";
import { Reader } from "./components/Reader";
import { Sidebar } from "./components/Sidebar";
import { Toasts } from "./components/Toasts";
import { openHandedOver } from "./lib/actions";
import { api } from "./lib/api";
import { comboMatches, runKey } from "./lib/commands";
import { composing } from "./lib/ime";
import { warn } from "./lib/log";
import { installMenuBar } from "./lib/menuBar";
import { loadAppVersion, scheduleUpdateCheck } from "./lib/update";
import { useStore } from "./store";

/** The focus is somewhere keys are for typing. */
const typingIn = (el: Element | null) => !!el?.closest("input, textarea, select, [contenteditable]");

/** Feeds are refreshed this long after launch (the window is up first). */
const FIRST_REFRESH_MS = 3000;

export default function App() {
  const refreshMinutes = useStore((s) => s.settings.refreshMinutes);

  useEffect(() => {
    void loadAppVersion();
    const uninstallMenu = installMenuBar();
    const stopUpdate = scheduleUpdateCheck();
    let unlisten: (() => void) | undefined;
    let unlistenIcons: (() => void) | undefined;
    let unlistenFullText: (() => void) | undefined;
    let unlistenOpen: (() => void) | undefined;
    const unlistenExtension: (() => void)[] = [];
    api
      .onRefreshProgress((p) => useStore.getState().setRefreshing(p))
      .then((u) => (unlisten = u))
      .catch((e) => warn("refresh progress", e));
    // Icons are looked up in the background, after a refresh or a subscription.
    api
      .onIconsChanged(() => void useStore.getState().reloadIcons())
      .then((u) => (unlistenIcons = u))
      .catch((e) => warn("icons changed", e));
    // Pages of 全文取得 arrive in the background too, one article at a time.
    api
      .onFullText((id) => void useStore.getState().loadFullText(id))
      .then((u) => (unlistenFullText = u))
      .catch((e) => warn("full text", e));
    // feed: links and .opml files from the OS: those of the launch are kept
    // in Rust until asked; later ones are told. Listening comes first, then
    // the first look: one handed over in between is told, not stranded.
    // Looks happen one after another, so none sees a request twice.
    let looking = Promise.resolve();
    const lookAtHandedOver = () =>
      (looking = looking.then(openHandedOver).catch((e) => warn("open requested", e)));
    let stopped = false;
    const listening = api
      .onOpenRequested(() => void lookAtHandedOver())
      // Unmounted before it was listening (React's dev double mount): stop at once.
      .then((u) => (stopped ? u() : (unlistenOpen = u)))
      .catch((e) => warn("open requested", e));
    void Promise.all([useStore.getState().init(), listening]).then(() => {
      if (!stopped) void lookAtHandedOver();
    });
    // The browser extension: its request to connect, and what it sent.
    for (const u of [
      api.onExtensionPair((r) => useStore.getState().setExtensionPair(r)),
      api.onExtensionPairEnd((id) => {
        if (useStore.getState().extensionPair?.id === id) useStore.getState().setExtensionPair(null);
      }),
      api.onExtensionChanged(async ({ message, feedId, laterId }) => {
        const s = useStore.getState();
        s.toast(message);
        await s.reloadSidebar();
        // The extension asked to show it (Rust has brought the window up).
        if (feedId !== null) return s.setView({ kind: "feed", id: feedId });
        if (laterId !== null) s.setView({ kind: "later" });
        await s.reload();
        if (laterId !== null) void s.select(laterId);
      }),
    ]) {
      // Unmounted before it was listening: stop at once, or every message shows twice.
      u.then((f) => (stopped ? f() : unlistenExtension.push(f))).catch((e) => warn("extension", e));
    }
    return () => {
      uninstallMenu();
      stopUpdate();
      unlisten?.();
      unlistenIcons?.();
      unlistenFullText?.();
      stopped = true;
      unlistenOpen?.();
      unlistenExtension.forEach((f) => f());
    };
  }, []);

  // The automatic refresh: shortly after launch, then every N minutes.
  useEffect(() => {
    if (refreshMinutes <= 0) return;
    const auto = () => void useStore.getState().refresh(undefined, true);
    const first = setTimeout(auto, FIRST_REFRESH_MS);
    const every = setInterval(auto, refreshMinutes * 60_000);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  }, [refreshMinutes]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || composing(e)) return;
      // ⌘, closes the settings as it opened them (desktop-platform UI §9.1).
      if (useStore.getState().dialog?.kind === "settings" && comboMatches("Mod+,", e)) {
        e.preventDefault();
        useStore.getState().closeDialog();
        return;
      }
      // A dialog or menu is open: it has the keys.
      if (useStore.getState().dialog || document.querySelector("[data-modal]")) return;
      const target = e.target as HTMLElement | null;
      // Enter / Space on a focused button press it.
      if (target instanceof HTMLButtonElement && (e.key === "Enter" || e.key === " ")) return;
      runKey(e, typingIn(document.activeElement));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div
      className="flex h-full"
      // The WebView's own menu (reload, inspect) only where text is: fields and the article, to copy.
      onContextMenu={(e) => {
        const el = e.target as Element;
        if (!typingIn(el) && !el.closest(".article")) e.preventDefault();
      }}
    >
      <Sidebar />
      <ArticleList />
      <Reader />
      <Dialogs />
      <ExtensionPairDialog />
      <Toasts />
    </div>
  );
}
