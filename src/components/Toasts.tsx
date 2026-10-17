import { CheckCircle2, Download, Info, Loader2, X, XCircle } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { useStore, type Toast, type ToastKind } from "../store";

const mb = (bytes: number) => (bytes / 1_000_000).toFixed(1);

const KIND: Record<ToastKind, { icon: typeof Info; color: string }> = {
  success: { icon: CheckCircle2, color: "var(--color-success)" },
  info: { icon: Info, color: "var(--color-accent)" },
  error: { icon: XCircle, color: "var(--color-danger)" },
};

/** Toasts wait while the app is in the background, so a result isn't missed. */
function useWindowActive() {
  const [active, setActive] = useState(() => document.hasFocus() && !document.hidden);
  useEffect(() => {
    const update = () => setActive(document.hasFocus() && !document.hidden);
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      window.removeEventListener("focus", update);
      window.removeEventListener("blur", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, []);
  return active;
}

const card = "pointer-events-auto overflow-hidden rounded-xl border bg-raised shadow-2xl shadow-black/50";

function ToastCard({ t, paused }: { t: Toast; paused: boolean }) {
  const dismiss = useStore((s) => s.dismissToast);
  const { icon: Icon, color } = KIND[t.kind];
  return (
    <div
      role={t.kind === "error" ? "alert" : "status"}
      className={`group relative ${card} ${t.leaving ? "animate-toast-out" : "animate-toast-in"}`}
      style={{ borderColor: `color-mix(in srgb, ${color} 45%, var(--color-line))` }}
    >
      <div
        key={`ring${t.rev}`}
        className="pointer-events-none absolute inset-0 rounded-xl animate-toast-ring"
        style={{ "--ring": `color-mix(in srgb, ${color} 55%, transparent)` } as React.CSSProperties}
      />
      <div className="flex items-start gap-3 py-3 pr-2 pl-3">
        <div
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full"
          style={{ background: `color-mix(in srgb, ${color} 20%, transparent)`, color }}
        >
          <Icon size={18} />
        </div>
        <div className="min-w-0 flex-1 pt-1.5">
          <div className="leading-snug font-semibold break-words">{t.title}</div>
          {t.detail && <div className="mt-0.5 text-xs leading-snug break-words whitespace-pre-line text-dim">{t.detail}</div>}
          {t.action && (
            <button
              onClick={() => {
                t.action!.onClick();
                dismiss(t.id);
              }}
              className="mt-2 rounded-md bg-accent px-3 py-1 text-xs font-medium text-white hover:brightness-110"
            >
              {t.action.label}
            </button>
          )}
        </div>
        <button onClick={() => dismiss(t.id)} className="shrink-0 rounded p-1 text-dim hover:bg-white/10 hover:text-fg">
          <X size={14} />
        </button>
      </div>
      {/* Time left; it ends the toast, and stops while hovered or in the background. */}
      <div
        key={`countdown${t.rev}`}
        className="h-0.5 origin-left animate-countdown group-hover:[animation-play-state:paused]"
        style={{
          background: color,
          opacity: 0.7,
          animationDuration: `${t.duration}ms`,
          animationPlayState: paused || t.leaving ? "paused" : undefined,
        }}
        onAnimationEnd={() => dismiss(t.id)}
      />
    </div>
  );
}

function ProgressCard({
  icon,
  label,
  right,
  ratio,
  onCancel,
}: {
  icon: React.ReactNode;
  label: string;
  right: string;
  /** 0..1, or null when the size isn't known yet. */
  ratio: number | null;
  onCancel?: () => void;
}) {
  return (
    <div className={`${card} animate-toast-in border-accent/40 p-3`}>
      <div className="mb-2 flex items-center gap-2">
        <span className="text-accent">{icon}</span>
        <span className="min-w-0 flex-1 truncate font-medium">{label}</span>
        <span className="shrink-0 text-xs tabular-nums text-dim">{right}</span>
        {onCancel && (
          <button onClick={onCancel} className="shrink-0 rounded px-2 py-0.5 text-xs text-dim hover:bg-white/10 hover:text-fg">
            中止
          </button>
        )}
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-bg">
        {ratio === null ? (
          <div className="h-full w-1/4 animate-indeterminate rounded-full bg-accent" />
        ) : (
          <div
            className="h-full rounded-full bg-accent transition-[width] duration-200 ease-linear"
            style={{ width: `${Math.min(100, ratio * 100)}%` }}
          />
        )}
      </div>
    </div>
  );
}

export function Toasts() {
  const toasts = useStore((s) => s.toasts);
  const refreshing = useStore((s) => s.refreshing);
  const updating = useStore((s) => s.updating);
  const active = useWindowActive();
  const pct = (r: number) => `${Math.floor(r * 100)}%`;
  return (
    <div className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-96 max-w-[calc(100vw-2rem)] flex-col gap-2">
      {refreshing && (
        <ProgressCard
          icon={<Loader2 size={16} className="animate-spin" />}
          label="フィードを更新しています…"
          right={`${refreshing.done} / ${refreshing.total}`}
          ratio={refreshing.total ? refreshing.done / refreshing.total : null}
          onCancel={() => void api.cancelRefresh()}
        />
      )}
      {updating && (
        <ProgressCard
          icon={<Download size={16} />}
          label={updating.installing ? "インストール中…" : "アップデートをダウンロード中…"}
          right={
            updating.total
              ? `${pct(updating.done / updating.total)}（${mb(updating.done)} / ${mb(updating.total)} MB）`
              : `${mb(updating.done)} MB`
          }
          ratio={updating.installing ? null : updating.total ? updating.done / updating.total : null}
        />
      )}
      {toasts.map((t) => (
        <ToastCard key={t.id} t={t} paused={!active} />
      ))}
    </div>
  );
}
