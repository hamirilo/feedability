/**
 * 外観 (docs/THEME.md): the page's colours follow `data-theme` on <html>
 * (index.css). The setting lives in settings.json; Rust puts the window in
 * the same theme (commands.rs `apply_theme`). The last resolved theme is
 * kept in localStorage too, so the page starts in it before the settings
 * have loaded.
 */
import { isTauri } from "@tauri-apps/api/core";
import type { ThemeSetting } from "./api";

const CACHE_KEY = "feedability.theme";
const dark = window.matchMedia("(prefers-color-scheme: dark)");
let setting: ThemeSetting = "system";

function resolve(): "light" | "dark" {
  if (setting === "system") return dark.matches ? "dark" : "light";
  return setting;
}

function paint() {
  const theme = resolve();
  document.documentElement.dataset.theme = theme;
  try {
    localStorage.setItem(CACHE_KEY, theme);
  } catch {
    /* private mode: the OS's theme until the settings load */
  }
}

/** Before the first render: the theme of the last session. */
export function paintCachedTheme() {
  let cached: string | null = null;
  try {
    cached = localStorage.getItem(CACHE_KEY);
  } catch {
    /* see paint */
  }
  document.documentElement.dataset.theme =
    cached === "light" || cached === "dark" ? cached : dark.matches ? "dark" : "light";
  // "System" follows the OS while the app runs.
  dark.addEventListener("change", () => setting === "system" && paint());
}

export function applyTheme(s: ThemeSetting) {
  setting = s;
  paint();
}

/**
 * 表示の大きさ (docs/UI_SCALE.md): Rust zooms the webview (commands.rs
 * `apply_ui_scale`), which grows everything in the page alike. The page only
 * keeps `--ui-zoom` for the room left for the window buttons, which don't
 * grow (index.css). In the browser (npm run dev) there is no Rust, so CSS
 * zoom stands in for it.
 */
const SCALE_KEY = "feedability.uiScale";

function paintUiScale(percent: number) {
  const zoom = percent / 100;
  const root = document.documentElement;
  root.style.setProperty("--ui-zoom", String(zoom));
  if (!isTauri()) root.style.zoom = zoom === 1 ? "" : String(zoom);
}

export function paintCachedUiScale() {
  let cached = NaN;
  try {
    cached = Number(localStorage.getItem(SCALE_KEY));
  } catch {
    /* see paint */
  }
  if (cached > 0) paintUiScale(cached);
}

export function applyUiScale(percent: number) {
  paintUiScale(percent);
  try {
    localStorage.setItem(SCALE_KEY, String(percent));
  } catch {
    /* see paint */
  }
}
