/**
 * The open article. Its HTML was sanitized in Rust (sanitize.rs); here it is
 * parsed into an inert document (DOMParser loads nothing), each image's
 * `src` is moved to `data-src`, and only then is it put on the page. Rust
 * fetches the images (`image_data_url`) and they come back as data: URLs:
 * the WebView itself never contacts the feed's servers.
 *
 * Links open in the OS's browser; the app's window never navigates.
 */
import { CircleDot, ExternalLink, Star } from "lucide-react";
import { useEffect, useRef } from "react";
import { openInBrowser } from "../lib/actions";
import { api } from "../lib/api";
import { command, isEnabled, runCommand } from "../lib/commands";
import { fullDate } from "../lib/format";
import { warn } from "../lib/log";
import { withKey } from "../lib/shortcuts";
import { dragRegion } from "../lib/titleBar";
import { useStore } from "../store";

/** Images fetched this session, by URL (the same logo in every article). */
const images = new Map<string, Promise<string>>();
const MAX_CACHED = 300;

function imageData(url: string): Promise<string> {
  let p = images.get(url);
  if (!p) {
    if (images.size >= MAX_CACHED) images.delete(images.keys().next().value!);
    p = api.imageDataUrl(url);
    // A failure isn't kept: the next article that has it tries again.
    p.catch(() => images.delete(url));
    images.set(url, p);
  }
  return p;
}

/** Fills the images of `root`, a few at a time, until `cancelled()` (another article). */
async function loadImages(root: HTMLElement, cancelled: () => boolean) {
  const queue = [...root.querySelectorAll<HTMLImageElement>("img[data-src]")];
  const worker = async () => {
    for (let img = queue.shift(); img; img = queue.shift()) {
      const src = img.dataset.src!;
      try {
        const data = await imageData(src);
        if (cancelled()) return;
        img.src = data;
        delete img.dataset.src;
      } catch (e) {
        if (cancelled()) return;
        // A tracking pixel or a dead link: gone rather than an empty box.
        warn(`image ${src}`, e);
        img.remove();
      }
    }
  };
  await Promise.all([worker(), worker(), worker(), worker()]);
}

function Body({ html }: { html: string }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const root = ref.current;
    if (!root) return;
    const doc = new DOMParser().parseFromString(html, "text/html");
    for (const img of doc.querySelectorAll("img")) {
      const src = img.getAttribute("src");
      img.removeAttribute("src");
      if (src && /^https?:/i.test(src)) img.dataset.src = src;
      else img.remove();
    }
    root.replaceChildren(...doc.body.childNodes);
    let stop = false;
    void loadImages(root, () => stop);
    return () => {
      stop = true;
    };
  }, [html]);
  const onClick = (e: React.MouseEvent) => {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const href = a.getAttribute("href");
    if (href) void openInBrowser(href);
  };
  // Middle-click and ⌘-click would otherwise open a new WebView window.
  const onAuxClick = (e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("a")) e.preventDefault();
  };
  return <div ref={ref} className="article" onClick={onClick} onAuxClick={onAuxClick} />;
}

function ToolButton({ id, icon, label, active }: { id: string; icon: React.ReactNode; label: string; active?: boolean }) {
  const enabled = useStore((s) => isEnabled(command(id), s));
  return (
    <button
      onClick={() => runCommand(id)}
      disabled={!enabled}
      title={withKey(label, id)}
      aria-label={label}
      aria-pressed={active}
      className={`rounded-md p-1.5 hover:bg-white/10 disabled:opacity-40 disabled:hover:bg-transparent ${
        active ? "text-star" : "text-dim hover:text-fg"
      }`}
    >
      {icon}
    </button>
  );
}

export function Reader() {
  const article = useStore((s) => s.article);
  const selectedId = useStore((s) => s.selectedId);
  const scroller = useRef<HTMLDivElement>(null);
  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 });
  }, [article?.id]);

  return (
    <main className="flex h-full min-w-0 flex-1 flex-col bg-bg">
      {/* The same three buttons whatever is open; greyed out with no article. */}
      <header
        {...dragRegion(true)}
        className="flex shrink-0 items-center justify-end gap-0.5 border-b border-line px-3"
        style={{ height: "max(var(--titlebar), 44px)" }}
      >
        <ToolButton id="article.read" icon={<CircleDot size={16} />} label={article && !article.read ? "既読にする" : "未読にする"} />
        <ToolButton
          id="article.star"
          icon={<Star size={16} className={article?.starred ? "fill-current" : ""} />}
          label={article?.starred ? "スターを外す" : "スターを付ける"}
          active={!!article?.starred}
        />
        <ToolButton id="article.open" icon={<ExternalLink size={16} />} label="ブラウザで開く" />
      </header>
      <div id="reader-scroll" ref={scroller} className="min-h-0 flex-1 overflow-y-auto">
        {article ? (
          <article className="mx-auto max-w-[720px] px-8 pt-8 pb-24">
            <div className="text-xs text-dim">{article.feedTitle}</div>
            <h1 className="mt-1 text-[22px] leading-snug font-bold select-text">
              {article.url ? (
                <a
                  href={article.url}
                  onClick={(e) => {
                    e.preventDefault();
                    void openInBrowser(article.url!);
                  }}
                  className="hover:text-accent"
                >
                  {article.title}
                </a>
              ) : (
                article.title
              )}
            </h1>
            <div className="mt-2 text-xs text-dim">
              {[article.author, fullDate(article.published)].filter(Boolean).join(" · ")}
            </div>
            <div className="mt-6">
              <Body html={article.html} />
            </div>
          </article>
        ) : (
          <p className="pt-32 text-center text-dim">{selectedId === null ? "記事を選んでください" : ""}</p>
        )}
      </div>
    </main>
  );
}
