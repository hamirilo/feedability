/**
 * macOS: the page runs under the title bar (tauri.conf.json: titleBarStyle
 * "Overlay"), with the window buttons over its top left. The page leaves room
 * for them (index.css: --titlebar, --traffic-lights) and its bars move the
 * window. The window is also transparent, for the sidebar's translucent
 * material (index.css: `.vibrant`). Windows keeps its own title bar and an
 * opaque window, so none of this applies there.
 */
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const overlayTitleBar = isTauri() && navigator.userAgent.includes("Mac");

/**
 * Props for a bar that moves the window when dragged (a double-click zooms
 * it). `deep`: its children too, except buttons, fields and links.
 */
export function dragRegion(deep = false) {
  return overlayTitleBar ? { "data-tauri-drag-region": deep ? "deep" : "" } : {};
}

/** Marks the page for index.css, and follows full screen (no window buttons). */
export function setUpTitleBar() {
  if (!overlayTitleBar) return;
  const root = document.documentElement;
  root.dataset.titlebar = "overlay";
  const win = getCurrentWindow();
  const update = () =>
    win
      .isFullscreen()
      .then((full) => root.toggleAttribute("data-fullscreen", full))
      .catch(() => {});
  update();
  win.onResized(update);
}
