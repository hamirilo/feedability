// In-app updates via tauri-plugin-updater.
//
// The installers are not Apple / Windows code-signed, so a freshly downloaded
// .dmg / .exe triggers Gatekeeper / SmartScreen every time. Files written by the
// app itself carry no quarantine attribute, so an update applied from inside the
// app launches without that warning. Only the first install still shows it.
import { getVersion } from "@tauri-apps/api/app";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { warn } from "./log";
import { useStore } from "../store";

const st = () => useStore.getState();

let version = "";
/** Current app version; empty until `loadAppVersion` resolves (and in the browser mock). */
export const appVersion = () => (version ? `v${version}` : "");

export async function loadAppVersion() {
  if (!isTauri()) return;
  try {
    version = await getVersion();
  } catch {
    /* dev mock */
  }
}

let busy = false;

/** Same as `plugins.updater.endpoints` in src-tauri/tauri.conf.json. */
const FEED = "https://github.com/hamirilo/feedability/releases/latest/download/latest.json";

/**
 * The proxy the OS would use for the feed (PAC included). The updater's HTTP
 * client can't evaluate PAC scripts, so behind a company proxy it would try to
 * connect directly and fail. Undefined = direct.
 */
async function systemProxy(): Promise<string | undefined> {
  try {
    return (await invoke<string | null>("system_proxy", { url: FEED })) ?? undefined;
  } catch {
    return undefined;
  }
}

/**
 * Check GitHub Releases for a newer version. `manual` reports "up to date" and
 * errors; the automatic startup check stays silent unless an update exists.
 */
export async function checkForUpdate(manual: boolean) {
  if (busy) return;
  if (!isTauri()) {
    if (manual) st().toast("ブラウザ版ではアップデートを確認できません", true);
    return;
  }
  busy = true;
  // One toast for the whole check: the result rewrites "確認しています…" in place.
  // It lasts past the check's timeout so it doesn't vanish before the result.
  const progress = manual ? st().notify({ title: "アップデートを確認しています…", duration: 30_000 }) : undefined;
  try {
    // The proxy also applies to the download (the Update keeps it).
    const update = await check({ timeout: 15_000, proxy: await systemProxy() });
    if (!update) {
      if (manual) st().notify({ title: "最新版です", detail: `Feedability ${appVersion()}`, kind: "success", replace: progress });
      return;
    }
    if (progress !== undefined) st().dismissToast(progress);
    await offer(update);
  } catch (e) {
    if (manual) st().notify({ title: "アップデートを確認できませんでした", detail: String(e), kind: "error", replace: progress });
    else warn("update check failed", e);
  } finally {
    busy = false;
  }
}

async function offer(update: Update) {
  const ok = await ask(
    `新しいバージョン v${update.version} があります（現在 v${update.currentVersion}）。` +
      "\n\nダウンロードして再起動しますか？",
    { title: "アップデート", kind: "info", okLabel: "更新して再起動", cancelLabel: "あとで" },
  );
  if (!ok) return;

  let done = 0;
  let total: number | null = null;
  st().setUpdating({ done, total });
  // Chunks arrive hundreds of times a second; re-rendering on each restarts
  // the bar's width transition every time, so it lagged and even ran backwards.
  let shown = 0;
  const show = (force = false) => {
    const now = performance.now();
    if (!force && now - shown < 120) return;
    shown = now;
    st().setUpdating({ done, total });
  };
  try {
    await update.downloadAndInstall((ev) => {
      if (ev.event === "Started") {
        total = ev.data.contentLength || null;
        show(true);
      } else if (ev.event === "Progress") {
        done += ev.data.chunkLength;
        show();
      } else if (ev.event === "Finished") {
        done = total ?? done;
        st().setUpdating({ done, total, installing: true });
      }
    });
  } catch (e) {
    st().setUpdating(null);
    await message(`アップデートに失敗しました。\n${String(e)}`, { title: "アップデート", kind: "error" });
    return;
  }
  st().setUpdating(null);
  // Windows: the installer has already asked the app to exit by now; relaunch is
  // a no-op there. macOS: the .app was swapped in place, restart to pick it up.
  await relaunch();
}

/**
 * Check a few seconds after launch in release builds, unless turned off in
 * the settings ("アップデートを自動で確認する"). Returns a cleanup.
 */
export function scheduleUpdateCheck(): () => void {
  if (import.meta.env.DEV || !isTauri()) return () => {};
  const t = setTimeout(() => {
    if (st().settings.autoUpdate) void checkForUpdate(false);
  }, 4000);
  return () => clearTimeout(t);
}
