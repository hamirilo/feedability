// The popup: the open page's feeds to subscribe to, and the page itself into
// あとで読む (src-tauri/src/extension.rs). The page is read only when the
// popup opens (activeTab), never in the background.
import { call, launchApp, openConnectPage, openInApp } from "./shared.js";

const $ = (id) => document.getElementById(id);
const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
const web = /^https?:/.test(tab?.url ?? "");

/** How the app answered: "ok", "unpaired" (401), "down" (not running or not receiving), "error". */
let app = "checking";

// ------------------------------------------------------------ the switches

const OPTIONS = { show: true, fullText: false };
const options = { ...OPTIONS, ...(await chrome.storage.local.get(Object.keys(OPTIONS))) };
for (const [key, id] of [
  ["show", "opt-show"],
  ["fullText", "opt-full"],
]) {
  $(id).checked = options[key];
  $(id).onchange = () => {
    options[key] = $(id).checked;
    void chrome.storage.local.set({ [key]: options[key] });
  };
}
/** The switches as the app reads them (extension.rs `Options`). */
const flags = (subscribing) => ({
  ...(options.show && { show: "1" }),
  ...(subscribing && options.fullText && { fullText: "1" }),
});

// ------------------------------------------------------------- what's said

let toastTimer;
function toast(text, kind = "ok") {
  const t = $("toast");
  t.hidden = false;
  t.className = `toast ${kind}`;
  $("toast-icon").setAttribute("href", kind === "ok" ? "#i-check" : "#i-alert");
  $("toast-text").textContent = text;
  clearTimeout(toastTimer);
  if (kind === "ok") toastTimer = setTimeout(() => (t.hidden = true), 6000);
}

/** A button's look while it works, and after. */
function busy(button, on) {
  button.disabled = on;
  if (on) {
    button.dataset.label = button.innerHTML;
    button.replaceChildren(Object.assign(document.createElement("span"), { className: "spinner" }));
  } else if (button.dataset.label) {
    button.innerHTML = button.dataset.label;
  }
}
function done(button, label) {
  button.disabled = true;
  button.classList.remove("primary");
  button.classList.add("done");
  button.innerHTML = `<svg class="i"><use href="#i-check" /></svg>`;
  button.append(label);
}

// ---------------------------------------------------------------- the page

function showPage() {
  let host = "";
  try {
    host = new URL(tab.url).host;
  } catch {
    /* no URL */
  }
  $("page-title").textContent = tab?.title || host || "このページ";
  $("page-host").textContent = web ? host : "Web のページではありません";
  const icon = $("favicon");
  icon.textContent = (host.replace(/^www\./, "")[0] || "?").toUpperCase();
  if (tab?.favIconUrl && /^(https?|data):/.test(tab.favIconUrl)) {
    const img = new Image();
    img.alt = "";
    img.onload = () => icon.replaceChildren(img);
    img.src = tab.favIconUrl;
  }
}

/** In the page: the feeds it names (`<link rel="alternate">`), as absolute URLs. */
function feedLinks() {
  const kinds = {
    "application/rss+xml": "RSS",
    "application/atom+xml": "Atom",
    "application/feed+json": "JSON",
    "application/json": "JSON",
  };
  return [...document.querySelectorAll('link[rel~="alternate"][href]')]
    .map((l) => ({ l, kind: kinds[(l.type || "").toLowerCase()] }))
    .filter(({ kind }) => kind)
    .map(({ l, kind }) => ({ url: new URL(l.getAttribute("href"), document.baseURI).href, title: l.title || "", kind }))
    .filter((f, i, all) => all.findIndex((g) => g.url === f.url) === i);
}

/** In the page: what it shows, for あとで読む. */
function pageHtml() {
  return { url: location.href, title: document.title, html: document.documentElement.outerHTML };
}

async function inPage(func) {
  const [r] = await chrome.scripting.executeScript({ target: { tabId: tab.id }, func });
  return r?.result;
}

// -------------------------------------------------------------- subscribing

const subscribeButtons = [];

async function subscribe(url, button, label = "購読しました") {
  // The app isn't up: the feed: link launches it with 「フィードを追加」 filled in.
  if (app === "down") {
    openInApp(url, tab.id);
    window.close();
    return;
  }
  busy(button, true);
  try {
    const { title } = await call("/subscribe", { method: "POST", params: { url, ...flags(true) } });
    busy(button, false);
    done(button, label);
    toast(`「${title}」を購読しました${options.fullText ? "（全文取得オン）" : ""}`);
  } catch (e) {
    busy(button, false);
    if (/すでに購読/.test(e.message)) {
      done(button, "購読済み");
      toast(e.message, "ok");
    } else {
      toast(e.message, "error");
      if (e.status === 0 || e.status === 401) void check();
    }
  }
}

/** A subscribe button whose label follows whether the app is up. */
function subscribeButton(url, primary) {
  const b = document.createElement("button");
  b.className = primary ? "btn primary" : "btn";
  b.onclick = () => subscribe(url, b);
  subscribeButtons.push(b);
  return b;
}

function labelButtons() {
  for (const b of subscribeButtons) {
    if (b.classList.contains("done")) continue;
    b.disabled = app === "unpaired" || app === "checking";
    b.innerHTML =
      app === "down"
        ? `<svg class="i"><use href="#i-launch" /></svg>アプリで開く`
        : `<svg class="i"><use href="#i-plus" /></svg>購読`;
    b.title = app === "down" ? "Feedability を起動して「フィードを追加」で開きます" : "";
  }
  $("try-page").disabled = app === "unpaired" || app === "checking";
  $("later").disabled = !web || app !== "ok";
}

async function showFeeds() {
  const feeds = web ? ((await inPage(feedLinks).catch(() => [])) ?? []) : [];
  const list = $("feeds");
  list.replaceChildren();
  for (const f of feeds) {
    const li = document.createElement("li");
    li.className = "feed";
    const kind = Object.assign(document.createElement("span"), { className: "kind", textContent: f.kind });
    const text = Object.assign(document.createElement("div"), { className: "text" });
    const title = Object.assign(document.createElement("span"), {
      className: "title ellipsis",
      textContent: f.title || tab.title || f.url,
    });
    const url = Object.assign(document.createElement("span"), { className: "url ellipsis", textContent: f.url });
    url.title = f.url;
    text.append(title, url);
    li.append(kind, text, subscribeButton(f.url, true));
    list.append(li);
  }
  $("feed-count").textContent = feeds.length > 1 ? `${feeds.length} 件` : "";
  $("no-feeds").hidden = feeds.length > 0;
  if (!web) {
    $("no-feeds-text").textContent = "Web のページを開いてから使ってください。";
    $("try-page").hidden = true;
  } else if (!feeds.length) {
    // Feedability looks for the page's feed itself, like 「フィードを追加」.
    const b = $("try-page");
    b.onclick = () => subscribe(tab.url, b);
  }
  labelButtons();
}

// --------------------------------------------------------------- あとで読む

$("later").onclick = async () => {
  const b = $("later");
  busy(b, true);
  try {
    const page = await inPage(pageHtml);
    await call("/later", {
      method: "POST",
      params: { url: page.url, title: page.title, ...flags(false) },
      body: page.html,
    });
    busy(b, false);
    done(b, "あとで読むに入れました");
    toast(`「${page.title || page.url}」をあとで読むに入れました`);
  } catch (e) {
    busy(b, false);
    toast(e.message, "error");
    if (e.status === 0 || e.status === 401) void check();
  }
};

// --------------------------------------------------------- the connection

function notice(title, text, buttons) {
  $("alert").hidden = !title;
  $("alert-title").textContent = title ?? "";
  $("alert-text").textContent = text ?? "";
  for (const id of ["connect", "launch", "recheck"]) $(id).hidden = !buttons?.includes(id);
}

$("connect").onclick = () => {
  openConnectPage();
  window.close();
};
$("launch").onclick = () => {
  launchApp(tab.id);
  // Look again once the app has had time to start (the popup stays open on
  // macOS while the OS asks; it closes if the app comes to the front).
  setTimeout(check, 4000);
};
$("recheck").onclick = () => check();

async function check() {
  const status = $("status");
  status.className = "pill";
  status.textContent = "確認中…";
  try {
    await call("/info");
    app = "ok";
    status.className = "pill ok";
    status.textContent = "接続中";
    notice(null);
  } catch (e) {
    status.className = "pill error";
    if (e.status === 401) {
      app = "unpaired";
      status.textContent = "未接続";
      notice(
        "アプリとまだ接続していません",
        "最初に 1 回だけ、この拡張機能を Feedability に登録します。アプリに 4 桁の番号が出るので、同じなら「許可する」を押してください。",
        ["connect"],
      );
    } else if (e.status === 0) {
      app = "down";
      status.textContent = "応答なし";
      notice(
        "Feedability が応答しません",
        "アプリが起動していないか、設定 › ブラウザ拡張 の受け付けがオフです。起動していなくても、フィードは「アプリで開く」で購読できます。",
        ["launch", "recheck"],
      );
    } else {
      app = "error";
      status.textContent = "エラー";
      notice("Feedability に接続できません", e.message, ["recheck"]);
    }
  }
  labelButtons();
}

showPage();
labelButtons();
await Promise.all([check(), showFeeds()]);
