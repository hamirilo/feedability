import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { command, isEnabled, labelOf, runCommand } from "../lib/commands";
import { comboText } from "../lib/shortcuts";
import { useStore } from "../store";

export interface MenuItem {
  label: string;
  hint?: string;
  disabled?: boolean;
  danger?: boolean;
  onClick: () => void;
}

/** A menu row for a command: its label now, its first key, greyed out when it can't run. */
export function commandItem(id: string, danger = false): MenuItem {
  const c = command(id);
  const s = useStore.getState();
  const key = c.keys?.[0];
  return {
    label: labelOf(c, s),
    hint: key ? comboText(key) : undefined,
    disabled: !isEnabled(c, s),
    danger,
    onClick: () => runCommand(id),
  };
}

export type MenuState = { x: number; y: number; items: (MenuItem | "sep")[] } | null;

/** A right-click menu, one level deep; kept inside the window. */
export function ContextMenu({ menu, onClose }: { menu: MenuState; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  useLayoutEffect(() => {
    if (!menu || !ref.current) return;
    const r = ref.current.getBoundingClientRect();
    setPos({
      x: Math.min(menu.x, window.innerWidth - r.width - 8),
      y: Math.min(menu.y, window.innerHeight - r.height - 8),
    });
  }, [menu]);
  useEffect(() => {
    if (!menu) return;
    const close = () => onClose();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", close);
    window.addEventListener("blur", close);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [menu, onClose]);
  if (!menu) return null;
  return (
    <div
      ref={ref}
      data-modal
      role="menu"
      className="fixed z-50 min-w-52 rounded-lg border border-line bg-raised py-1 shadow-2xl shadow-black/60"
      style={{ left: pos.x, top: pos.y }}
      onMouseDown={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.preventDefault()}
    >
      {menu.items.map((item, i) =>
        item === "sep" ? (
          <div key={i} className="my-1 border-t border-line" />
        ) : (
          <button
            key={i}
            role="menuitem"
            disabled={item.disabled}
            onClick={() => {
              onClose();
              item.onClick();
            }}
            className={`flex w-full items-center justify-between gap-6 px-3 py-1 text-left enabled:hover:bg-accent enabled:hover:text-white disabled:opacity-40 ${
              item.danger ? "text-danger" : ""
            }`}
          >
            <span>{item.label}</span>
            {item.hint && <span className="text-xs opacity-60">{item.hint}</span>}
          </button>
        ),
      )}
    </div>
  );
}
