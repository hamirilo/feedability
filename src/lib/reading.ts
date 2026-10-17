/** The reader's display steps (docs/READING.md). Indexes are what is saved. */
export const TEXT_SIZES = [13, 14, 15, 17, 19];
export const LINE_HEIGHTS = [
  { value: 1.6, label: "狭い" },
  { value: 1.8, label: "標準" },
  { value: 2.0, label: "広い" },
];
export const TEXT_WIDTHS = [
  { value: 600, label: "狭い" },
  { value: 720, label: "標準" },
  { value: 880, label: "広い" },
];

export const DEFAULT_TEXT_SIZE = 2;
export const DEFAULT_LINE_HEIGHT = 1;
export const DEFAULT_TEXT_WIDTH = 1;

/** A saved index, or `fallback` when it is not one of the steps (a hand-edited file). */
export function validStep(v: unknown, count: number, fallback: number): number {
  return typeof v === "number" && Number.isInteger(v) && v >= 0 && v < count ? v : fallback;
}
