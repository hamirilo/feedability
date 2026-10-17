/**
 * The widths of the sidebar and the article list (docs/LAYOUT.md). Pure, so
 * the limits read the same in the store, the handles and the tests.
 */

export type Pane = "sidebar" | "list";

export const PANE_DEFAULT: Record<Pane, number> = { sidebar: 240, list: 380 };
export const PANE_MIN: Record<Pane, number> = { sidebar: 180, list: 280 };
export const PANE_MAX: Record<Pane, number> = { sidebar: 400, list: 640 };
/** The reader never gets narrower than this, whatever the panes ask for. */
export const READER_MIN = 360;

const clamp = (v: number, lo: number, hi: number) => Math.round(Math.max(lo, Math.min(hi, v)));

export interface PaneWidths {
  sidebar: number;
  list: number;
  /** What each can grow to now (the window and the other pane considered). */
  sidebarMax: number;
  listMax: number;
}

/**
 * The widths to draw for the saved ones in a window `inner` px wide. The
 * saved values are not changed: the window growing again gives the pane its
 * width back.
 */
export function paneWidths(saved: { sidebar: number; list: number }, inner: number): PaneWidths {
  // The sidebar goes first and leaves the list its minimum; then the list takes what the sidebar left.
  const sidebarMax = Math.max(PANE_MIN.sidebar, Math.min(PANE_MAX.sidebar, inner - PANE_MIN.list - READER_MIN));
  const sidebar = clamp(saved.sidebar, PANE_MIN.sidebar, sidebarMax);
  const listMax = Math.max(PANE_MIN.list, Math.min(PANE_MAX.list, inner - sidebar - READER_MIN));
  const list = clamp(saved.list, PANE_MIN.list, listMax);
  // The sidebar's limit while dragging it: what the list now takes is kept.
  return { sidebar, list, sidebarMax: Math.max(PANE_MIN.sidebar, Math.min(PANE_MAX.sidebar, inner - list - READER_MIN)), listMax };
}

/** A width read from the saved prefs: a number inside the pane's limits, else the default. */
export function savedWidth(pane: Pane, v: unknown): number {
  return typeof v === "number" && Number.isFinite(v) ? clamp(v, PANE_MIN[pane], PANE_MAX[pane]) : PANE_DEFAULT[pane];
}

export const clampWidth = (pane: Pane, v: number, max: number) => clamp(v, PANE_MIN[pane], Math.max(PANE_MIN[pane], max));
