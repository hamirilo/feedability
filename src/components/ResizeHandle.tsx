/**
 * The drag handle on a pane's right edge (docs/LAYOUT.md): pointer events
 * with the pointer captured, so the drag follows the mouse out of the 5px
 * strip. ←/→ change the width by 16px, a double click resets it.
 */
import { useEffect, useRef, useState } from "react";
import { clampWidth, paneWidths, type Pane, type PaneWidths } from "../lib/layout";
import { useStore } from "../store";

const STEP = 16;
const LABEL: Record<Pane, string> = { sidebar: "サイドバーの幅", list: "記事一覧の幅" };

/** The widths to draw: the saved ones, kept inside what the window leaves. */
export function usePaneWidths(): PaneWidths {
  const sidebar = useStore((s) => s.sidebarWidth);
  const list = useStore((s) => s.listWidth);
  const [inner, setInner] = useState(window.innerWidth);
  useEffect(() => {
    const on = () => setInner(window.innerWidth);
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  return paneWidths({ sidebar, list }, inner);
}

export function ResizeHandle({ pane }: { pane: Pane }) {
  const widths = usePaneWidths();
  const value = widths[pane];
  const max = pane === "sidebar" ? widths.sidebarMax : widths.listMax;
  const setPaneWidth = useStore((s) => s.setPaneWidth);
  const savePaneWidths = useStore((s) => s.savePaneWidths);
  const resetPaneWidth = useStore((s) => s.resetPaneWidth);
  const [dragging, setDragging] = useState(false);
  const start = useRef<{ x: number; width: number } | null>(null);

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    // Keeps the drag from selecting text on the way.
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    start.current = { x: e.clientX, width: value };
    setDragging(true);
  };
  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const s = start.current;
    if (s) setPaneWidth(pane, clampWidth(pane, s.width + e.clientX - s.x, max));
  };
  const end = () => {
    if (!start.current) return;
    start.current = null;
    setDragging(false);
    // Saved once, not on every move.
    savePaneWidths();
  };
  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const delta = e.key === "ArrowLeft" ? -STEP : e.key === "ArrowRight" ? STEP : 0;
    if (!delta) return;
    e.preventDefault();
    setPaneWidth(pane, clampWidth(pane, value + delta, max));
    savePaneWidths();
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={LABEL[pane]}
      aria-valuenow={value}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={end}
      onPointerCancel={end}
      onLostPointerCapture={end}
      onDoubleClick={() => resetPaneWidth(pane)}
      onKeyDown={onKeyDown}
      // Over the border, 2px of it outside the pane and 3px in.
      className="group absolute inset-y-0 -right-0.5 z-20 w-[5px] cursor-col-resize touch-none outline-none select-none"
    >
      <span
        className={`absolute inset-y-0 left-[2px] w-px transition-colors ${
          dragging ? "bg-accent" : "bg-transparent group-hover:bg-accent/60 group-focus-visible:bg-accent"
        }`}
      />
    </div>
  );
}
