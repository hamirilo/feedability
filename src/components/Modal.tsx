import { useEffect, useRef } from "react";
import { composing } from "../lib/ime";

/**
 * A dialog over the window. `data-modal` stops the window's key handler
 * (App.tsx) while it is open; Esc closes it unless an IME is converting.
 */
export function Modal({
  title,
  onClose,
  children,
  width = 440,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
  width?: number;
}) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !composing(e)) {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    // The first field, else the dialog itself, so keys go to it.
    const first = box.current?.querySelector<HTMLElement>("input, select, textarea");
    (first ?? box.current)?.focus();
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div
      data-modal
      className="fixed inset-0 z-40 flex animate-fade-in items-start justify-center bg-black/50 pt-[12vh]"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div
        ref={box}
        tabIndex={-1}
        role="dialog"
        aria-label={title}
        className="max-h-[76vh] animate-zoom-in overflow-y-auto rounded-xl border border-line bg-panel p-5 shadow-2xl shadow-black/60 outline-none"
        style={{ width, maxWidth: "calc(100vw - 2rem)" }}
      >
        <h2 className="mb-4 text-[15px] font-semibold">{title}</h2>
        {children}
      </div>
    </div>
  );
}

export const field =
  "w-full rounded-md border border-line bg-raised px-2.5 py-1.5 text-[13px] outline-none focus:border-accent";

export function Buttons({
  okLabel,
  onCancel,
  busy = false,
  disabled = false,
}: {
  okLabel: string;
  onCancel: () => void;
  busy?: boolean;
  disabled?: boolean;
}) {
  return (
    <div className="mt-5 flex justify-end gap-2">
      <button type="button" onClick={onCancel} className="rounded-md px-3 py-1.5 text-dim hover:bg-hover hover:text-fg">
        キャンセル
      </button>
      <button
        type="submit"
        disabled={busy || disabled}
        // A fixed minimum width: "追加" and "追加しています…" don't move the buttons.
        className="min-w-24 rounded-md bg-accent px-3 py-1.5 font-medium text-white hover:brightness-110 disabled:opacity-50"
      >
        {busy ? `${okLabel}しています…` : okLabel}
      </button>
    </div>
  );
}
