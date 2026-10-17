import { keyParts, shortcutSections } from "../lib/shortcuts";
import { Modal } from "./Modal";

function Keys({ combo }: { combo: string }) {
  return (
    <span className="inline-flex gap-0.5">
      {keyParts(combo).map((k, i) => (
        <kbd key={i} className="min-w-5 rounded border border-line bg-raised px-1.5 py-0.5 text-center font-sans text-[11px]">
          {k}
        </kbd>
      ))}
    </span>
  );
}

/** ? : every key, from the command table. */
export function ShortcutHelp({ onClose }: { onClose: () => void }) {
  return (
    <Modal title="キーボードショートカット" onClose={onClose} width={560}>
      <div className="grid gap-5">
        {shortcutSections().map((sec) => (
          <section key={sec.title}>
            <h3 className="mb-1.5 text-xs font-semibold text-dim">{sec.title}</h3>
            {sec.rows.map((r) => (
              <div key={r.label} className="flex items-center justify-between gap-4 border-b border-line/60 py-1.5">
                <span>{r.label}</span>
                <span className="flex shrink-0 gap-2">
                  {r.keys.map((k) => (
                    <Keys key={k} combo={k} />
                  ))}
                </span>
              </div>
            ))}
          </section>
        ))}
        <p className="text-xs text-dim">1 文字のキー（J、K、M など）は、入力欄の外で使えます。</p>
      </div>
    </Modal>
  );
}
