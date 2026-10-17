/**
 * 外観 (docs/THEME.md): the page's colours follow `data-theme` on <html>
 * (index.css). The setting lives in settings.json; Rust puts the window in
 * the same theme (commands.rs `apply_theme`). The last resolved theme is
 * kept in localStorage too, so the page starts in it before the settings
 * have loaded.
 */
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
