/**
 * Warnings and errors into the app's log file as well as the console
 * (tauri-plugin-log, set up in src-tauri/src/lib.rs), so a problem on a
 * user's PC can be looked at afterwards: ヘルプ > ログを表示.
 */
import { isTauri } from "@tauri-apps/api/core";
import { error as logError, warn as logWarn } from "@tauri-apps/plugin-log";

function text(message: string, detail: unknown): string {
  if (detail === undefined) return message;
  if (detail instanceof Error) return `${message}: ${detail.stack ?? detail.message}`;
  if (typeof detail === "string") return `${message}: ${detail}`;
  try {
    return `${message}: ${JSON.stringify(detail)}`;
  } catch {
    return `${message}: ${String(detail)}`;
  }
}

export function warn(message: string, detail?: unknown) {
  if (detail === undefined) console.warn(message);
  else console.warn(message, detail);
  if (isTauri()) logWarn(text(message, detail)).catch(() => {});
}

/** Errors nothing caught (a failed render, a rejected promise) into the log. */
export function logUncaughtErrors() {
  if (!isTauri()) return;
  window.addEventListener("error", (e) => {
    logError(text("uncaught", e.error ?? e.message)).catch(() => {});
  });
  window.addEventListener("unhandledrejection", (e) => {
    logError(text("unhandled rejection", e.reason)).catch(() => {});
  });
}
