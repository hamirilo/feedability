/** The list's layouts (docs/LIST_LAYOUT.md). What is saved is the id. */
export type ListLayout = "standard" | "compact" | "thumbs";

export const LIST_LAYOUTS: { id: ListLayout; label: string; row: number }[] = [
  { id: "standard", label: "標準", row: 92 },
  { id: "compact", label: "コンパクト", row: 34 },
  { id: "thumbs", label: "画像付き", row: 92 },
];

export const DEFAULT_LIST_LAYOUT: ListLayout = "standard";

/** A saved layout, or the default when it is not one (a hand-edited file). */
export function validLayout(v: unknown): ListLayout {
  return LIST_LAYOUTS.some((l) => l.id === v) ? (v as ListLayout) : DEFAULT_LIST_LAYOUT;
}

export const rowHeight = (layout: ListLayout) => LIST_LAYOUTS.find((l) => l.id === layout)!.row;
