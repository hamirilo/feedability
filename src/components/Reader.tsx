/**
 * The open article. Its HTML was sanitized in Rust (sanitize.rs); here it is
 * parsed into an inert document (DOMParser loads nothing), each image's
 * `src` is moved to `data-src`, and only then is it put on the page. Rust
 * fetches the images (`image_data_url`) and they come back as data: URLs:
 * the WebView itself never contacts the feed's servers.
 *
 * Links open in the OS's browser; the app's window never navigates.
 */
import { Archive, CircleDot, Clock, ExternalLink, Minus, Plus, Star, Type } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { openInBrowser } from "../lib/actions";
import { api } from "../lib/api";
import { command, isEnabled, runCommand } from "../lib/commands";
import { fullDate } from "../lib/format";
import { warn } from "../lib/log";
import { LINE_HEIGHTS, TEXT_SIZES, TEXT_WIDTHS } from "../lib/reading";
import { withKey } from "../lib/shortcuts";
import { dragRegion } from "../lib/titleBar";
import { useStore } from "../store";
import { FeedIcon } from "./FeedIcon";

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

/**
 * The same picture by file, whatever size the site asked for: WordPress
 * names its sizes `photo-1024x576.png` next to `photo.png`. The query is
 * kept: some sites tell their pictures apart only by it.
 */
function imageKey(src: string): string {
  return src.replace(/#.*$/, "").replace(/-\d+x\d+(\.\w+)(\?|$)/, "$1$2");
}

/**
 * Drops an image already shown higher up. Some feeds put the post's featured
 * image before a body that opens with the same picture (kachibito.net), so it
 * shows twice. Only a picture on its own goes: an emoji repeated in a
 * sentence stays. A wrapper left empty (`<p>`, `<a>`, `<figure>`) goes too.
 */
function dropRepeatedImages(doc: Document) {
  const seen = new Set<string>();
  for (const img of doc.querySelectorAll<HTMLImageElement>("img[data-src]")) {
    const key = imageKey(img.dataset.src!);
    const repeated = seen.has(key);
    seen.add(key);
    if (!repeated || img.parentElement?.textContent?.trim()) continue;
    let node: Element = img;
    while (
      node.parentElement &&
      node.parentElement !== doc.body &&
      node.parentElement.childElementCount === 1 &&
      !node.parentElement.textContent?.trim()
    ) {
      node = node.parentElement;
    }
    node.remove();
  }
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
    dropRepeatedImages(doc);
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

function ToolButton({
  id,
  icon,
  label,
  active,
}: {
  id: string;
  icon: React.ReactNode;
  label: string;
  /** The colour when on (a star is yellow, あとで読む blue). */
  active?: string;
}) {
  const enabled = useStore((s) => isEnabled(command(id), s));
  return (
    <button
      onClick={() => runCommand(id)}
      disabled={!enabled}
      title={withKey(label, id)}
      aria-label={label}
      aria-pressed={active !== undefined}
      className={`rounded-md p-1.5 hover:bg-hover disabled:opacity-40 disabled:hover:bg-transparent ${
        active ?? "text-dim hover:text-fg"
      }`}
    >
      {icon}
    </button>
  );
}

function Steps({
  label,
  steps,
  value,
  onPick,
}: {
  label: string;
  steps: { label: string }[];
  value: number;
  onPick: (i: number) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="text-dim">{label}</span>
      <div className="flex rounded-md bg-bg p-0.5 text-xs" role="radiogroup" aria-label={label}>
        {steps.map((st, i) => (
          <button
            key={st.label}
            role="radio"
            aria-checked={value === i}
            onClick={() => onPick(i)}
            className={`rounded px-2.5 py-0.5 ${value === i ? "bg-raised text-fg shadow" : "text-dim hover:text-fg"}`}
          >
            {st.label}
          </button>
        ))}
      </div>
    </div>
  );
}

/** 「Aa」: the display panel (docs/READING.md). Not a modal: the keys keep working. */
function DisplayButton() {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  const textSize = useStore((s) => s.textSize);
  const lineHeight = useStore((s) => s.lineHeight);
  const textWidth = useStore((s) => s.textWidth);
  const smaller = useStore((s) => isEnabled(command("view.textSmaller"), s));
  const bigger = useStore((s) => isEnabled(command("view.textBigger"), s));
  const setLineHeight = useStore((s) => s.setLineHeight);
  const setTextWidth = useStore((s) => s.setTextWidth);
  const resetReading = useStore((s) => s.resetReading);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!box.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    const close = () => setOpen(false);
    window.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", close);
    return () => {
      window.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", close);
    };
  }, [open]);
  const step = "rounded-md p-1.5 text-dim hover:bg-hover hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent";
  return (
    <div ref={box} className="relative">
      <button
        onClick={() => setOpen(!open)}
        title="文字と表示"
        aria-label="文字と表示"
        aria-expanded={open}
        className={`rounded-md p-1.5 hover:bg-hover ${open ? "bg-hover text-fg" : "text-dim hover:text-fg"}`}
      >
        <Type size={16} />
      </button>
      {open && (
        <div data-tauri-drag-region="false" role="dialog" aria-label="文字と表示" className="absolute top-full right-0 z-40 mt-1 w-72 space-y-3 rounded-lg border border-line bg-raised p-3 shadow-2xl shadow-black/60">
          <div className="flex items-center justify-between gap-4">
            <span className="text-dim">文字の大きさ</span>
            <div className="flex items-center gap-1">
              <button className={step} disabled={!smaller} onClick={() => runCommand("view.textSmaller")} title={withKey("文字を小さく", "view.textSmaller")} aria-label="文字を小さく">
                <Minus size={14} />
              </button>
              <span className="w-12 text-center tabular-nums">{TEXT_SIZES[textSize]} px</span>
              <button className={step} disabled={!bigger} onClick={() => runCommand("view.textBigger")} title={withKey("文字を大きく", "view.textBigger")} aria-label="文字を大きく">
                <Plus size={14} />
              </button>
            </div>
          </div>
          <Steps label="行間" steps={LINE_HEIGHTS} value={lineHeight} onPick={setLineHeight} />
          <Steps label="本文の幅" steps={TEXT_WIDTHS} value={textWidth} onPick={setTextWidth} />
          <div className="text-right">
            <button onClick={resetReading} className="text-xs text-dim hover:text-fg">
              標準に戻す
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export function Reader() {
  const article = useStore((s) => s.article);
  const selectedId = useStore((s) => s.selectedId);
  const textSize = useStore((s) => s.textSize);
  const lineHeight = useStore((s) => s.lineHeight);
  const textWidth = useStore((s) => s.textWidth);
  const scroller = useRef<HTMLDivElement>(null);
  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 });
  }, [article?.id]);

  return (
    <main className="flex h-full min-w-0 flex-1 flex-col bg-bg">
      {/* The same buttons whatever is open; greyed out with no article. */}
      <header
        {...dragRegion(true)}
        className="flex shrink-0 items-center justify-end gap-0.5 border-b border-line px-3"
        style={{ height: "max(var(--titlebar), 44px)" }}
      >
        <DisplayButton />
        <ToolButton id="article.read" icon={<CircleDot size={16} />} label={article && !article.read ? "既読にする" : "未読にする"} />
        <ToolButton
          id="article.star"
          icon={<Star size={16} className={article?.starred ? "fill-current" : ""} />}
          label={article?.starred ? "スターを外す" : "スターを付ける"}
          active={article?.starred ? "text-star" : undefined}
        />
        <ToolButton
          id="article.later"
          icon={<Clock size={16} />}
          label={article?.later ? "あとで読むから外す" : "あとで読む"}
          active={article?.later ? "text-accent" : undefined}
        />
        <ToolButton id="article.open" icon={<ExternalLink size={16} />} label="ブラウザで開く" />
        {/* Last, after the ones that were there: they keep their places (UI Standard §10). */}
        <ToolButton id="article.stockroom" icon={<Archive size={16} />} label="stockroom に送る" />
      </header>
      <div id="reader-scroll" ref={scroller} className="min-h-0 flex-1 overflow-y-auto">
        {article ? (
          <article
            className="mx-auto px-8 pt-8 pb-24"
            style={
              {
                maxWidth: TEXT_WIDTHS[textWidth].value,
                "--article-size": `${TEXT_SIZES[textSize]}px`,
                "--article-leading": LINE_HEIGHTS[lineHeight].value,
              } as React.CSSProperties
            }
          >
            <div className="flex items-center gap-1.5 text-xs text-dim">
              <FeedIcon feedId={article.feedId} size={12} />
              <span className="min-w-0 truncate">{article.feedTitle}</span>
            </div>
            <h1
              className="mt-1 leading-snug font-bold select-text"
              style={{ fontSize: "calc(var(--article-size) * 22 / 15)" }}
            >
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
            {/* 全文取得 (docs/FULL_TEXT.md): where the text comes from, under the date. Nothing moves when it arrives but the text. */}
            {article.fullText === "pending" && <div className="mt-1 text-xs text-dim">記事のページから本文を取得しています…</div>}
            {article.fullText === "page" && <div className="mt-1 text-xs text-dim">記事のページから取得した本文</div>}
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
